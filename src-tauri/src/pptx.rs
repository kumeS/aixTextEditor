//! PPTX (Office Open XML) export, written by hand (no external pptx crate).
//!
//! A `Deck` is serialized into a minimal, valid `.pptx` zip that opens in
//! PowerPoint / Keynote / Google Slides. We emit explicit absolute-positioned
//! shapes (`p:sp` text boxes, `p:pic` images) per slide rather than relying on
//! slide-master placeholder inheritance — the simplest robust path, and the
//! text stays editable. Geometry is computed in code (EMU); the layout template
//! is chosen upstream in `deck::document_to_deck`.

use crate::error::{AppError, AppResult};
use crate::models::{Chunk, Deck, Slide, CHUNK_TYPE_TEXT};
use base64::Engine;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

// English Metric Units. 914400 EMU = 1 inch, 12700 = 1 pt.
const SLIDE_W: i64 = 12_192_000; // 16:9
const SLIDE_H: i64 = 6_858_000;
const MARGIN: i64 = 685_800; // 0.75 in
const BODY_Y: i64 = 1_600_200;

/// Upper bound on a single fetched remote image (A4): a hostile or accidentally
/// huge URL can't exhaust memory during export.
const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;

/// Outcome of an export: how many slides were written and any non-fatal notes
/// (e.g. images that couldn't be embedded, diagram chunks not yet supported).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxReport {
    pub slides: usize,
    pub warnings: Vec<String>,
}

/// Resolve remote (`http(s)://`) image-chunk URLs to inline data URLs by
/// fetching the bytes, so the (synchronous) writer can embed them. Image chunks
/// can hold a remote URL (some image models return a hosted URL rather than a
/// data URL — see `ai::extract_image_url`). On failure the content is cleared so
/// the writer skips it and reports it as a dropped image.
pub async fn resolve_remote_images(deck: &mut Deck) {
    for slide in &mut deck.slides {
        for chunk in &mut slide.chunks {
            if !chunk.is_image() {
                continue;
            }
            let url = chunk.content.trim();
            if url.starts_with("http://") || url.starts_with("https://") {
                // On failure the content is cleared → counted as a dropped image.
                chunk.content = fetch_as_data_url(url).await.unwrap_or_default();
            }
        }
    }
}

async fn fetch_as_data_url(url: &str) -> AppResult<String> {
    // `net::safe_fetch` enforces http(s)-only, SSRF host filtering, per-hop
    // redirect re-validation, a size cap and a timeout (A4/A5).
    let bytes = crate::net::safe_fetch(url, MAX_IMAGE_BYTES, 30).await?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    // The mime here is cosmetic — `image_ext` re-sniffs the magic bytes on write.
    Ok(format!("data:image/png;base64,{b64}"))
}

/// Build the `.pptx` bytes for a deck, plus any non-fatal warnings. Call
/// `resolve_remote_images` first so remote image URLs become embeddable.
pub fn deck_to_pptx(deck: &Deck) -> AppResult<(Vec<u8>, Vec<String>)> {
    let n = deck.slides.len();

    // Tally what the layout can't carry yet, so the caller can surface it. Image
    // outcomes (failed fetch / unsupported format / extras) and bullet overflow
    // are recorded per-slide in `build_slide`; diagrams are counted up front.
    let diagrams: usize = deck
        .slides
        .iter()
        .flat_map(|s| &s.chunks)
        .filter(|c| c.is_diagram())
        .count();
    let mut stats = ExportStats::default();
    let mut buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut buf);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let add = |zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>, name: &str, data: &[u8]| -> AppResult<()> {
        zip.start_file(name, opts)
            .map_err(|e| AppError::Other(format!("PPTX zip error: {e}")))?;
        zip.write_all(data)?;
        Ok(())
    };

    add(&mut zip, "[Content_Types].xml", content_types(n).as_bytes())?;
    add(&mut zip, "_rels/.rels", PACKAGE_RELS.as_bytes())?;
    add(&mut zip, "docProps/core.xml", core_xml(&deck.title).as_bytes())?;
    add(&mut zip, "docProps/app.xml", app_xml(n).as_bytes())?;
    add(&mut zip, "ppt/presentation.xml", presentation_xml(n).as_bytes())?;
    add(
        &mut zip,
        "ppt/_rels/presentation.xml.rels",
        presentation_rels(n).as_bytes(),
    )?;
    add(&mut zip, "ppt/theme/theme1.xml", THEME.as_bytes())?;
    add(
        &mut zip,
        "ppt/slideMasters/slideMaster1.xml",
        SLIDE_MASTER.as_bytes(),
    )?;
    add(
        &mut zip,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        SLIDE_MASTER_RELS.as_bytes(),
    )?;
    add(
        &mut zip,
        "ppt/slideLayouts/slideLayout1.xml",
        SLIDE_LAYOUT.as_bytes(),
    )?;
    add(
        &mut zip,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        SLIDE_LAYOUT_RELS.as_bytes(),
    )?;

    let mut media_counter = 0usize;
    for (i, slide) in deck.slides.iter().enumerate() {
        let (sp_tree, images) = build_slide(slide, &mut media_counter, &mut stats);
        let n1 = i + 1;
        add(
            &mut zip,
            &format!("ppt/slides/slide{n1}.xml"),
            slide_xml(&sp_tree).as_bytes(),
        )?;
        add(
            &mut zip,
            &format!("ppt/slides/_rels/slide{n1}.xml.rels"),
            slide_rels(&images).as_bytes(),
        )?;
        for img in &images {
            add(&mut zip, &format!("ppt/media/{}", img.file), &img.bytes)?;
        }
    }

    zip.finish()
        .map_err(|e| AppError::Other(format!("PPTX zip error: {e}")))?;

    let mut warnings = Vec::new();
    if stats.fetch_failed > 0 {
        warnings.push(format!(
            "{} image(s) couldn't be downloaded and were left out.",
            stats.fetch_failed
        ));
    }
    if stats.unsupported_format > 0 {
        warnings.push(format!(
            "{} image(s) use a format PowerPoint can't embed (e.g. WEBP or SVG) and were left out.",
            stats.unsupported_format
        ));
    }
    if stats.extra_images > 0 {
        warnings.push(format!(
            "{} extra image(s) were left out — only one image per slide is supported for now.",
            stats.extra_images
        ));
    }
    if stats.overflow_slides > 0 {
        warnings.push(format!(
            "{} slide(s) have more text than fits and may be cut off — consider splitting them.",
            stats.overflow_slides
        ));
    }
    if diagrams > 0 {
        warnings.push(format!(
            "{diagrams} diagram(s) were left out — diagram export to PPTX is coming in a later update."
        ));
    }
    Ok((buf.into_inner(), warnings))
}

// ----- per-slide shape tree -------------------------------------------------

struct SlideImage {
    rid: String,
    file: String,
    bytes: Vec<u8>,
}

/// Per-export tally of content that the layout couldn't carry, so the caller can
/// surface a *specific* warning for each cause (A7) instead of one conflated note.
#[derive(Default)]
struct ExportStats {
    /// Image chunks whose bytes couldn't be decoded (e.g. a remote fetch failed
    /// and the content was cleared).
    fetch_failed: usize,
    /// Image chunks in a format PowerPoint can't embed (WEBP/SVG/unknown).
    unsupported_format: usize,
    /// Images dropped because the slide already had one (one image per slide).
    extra_images: usize,
    /// Slides whose estimated bullet text likely overflows the body box.
    overflow_slides: usize,
}

fn is_text(c: &Chunk) -> bool {
    c.metadata.chunk_type == CHUNK_TYPE_TEXT
}

fn build_slide(
    slide: &Slide,
    media_counter: &mut usize,
    stats: &mut ExportStats,
) -> (String, Vec<SlideImage>) {
    let mut shapes = String::new();
    let mut images = Vec::new();
    let mut sid: u32 = 2; // shape id 1 is the group

    let heading = slide
        .chunks
        .iter()
        .find(|c| c.is_heading())
        .map(|c| c.content.clone());
    // Req 3: an explicit subtitle chunk (section falls back to the first bullet).
    let subtitle_text: Option<String> = slide
        .chunks
        .iter()
        .find(|c| c.is_subtitle())
        .map(|c| c.content.clone());
    // Req 2: a "detached" slide renders its own `slideBody` lines (a summary /
    // custom content) instead of the linked editor paragraphs.
    let slide_body: Option<Vec<String>> =
        slide.chunks.iter().find_map(|c| c.metadata.slide_body.clone());
    let bullet_texts: Vec<String> = match &slide_body {
        Some(body) => body.clone(),
        None => slide
            .chunks
            .iter()
            .filter(|c| is_text(c) && !c.is_subtitle())
            .map(|c| c.content.clone())
            .collect(),
    };
    let imgs: Vec<&Chunk> = slide.chunks.iter().filter(|c| c.is_image()).collect();

    // A7: estimate whether the bullets overflow the body box (bullet layouts
    // only). A char-count heuristic — approximate, but enough to warn the user
    // that text may be clipped so they can split the slide.
    if slide.layout != "section" {
        let cpl = if slide.layout == "title-image" { 60 } else { 110 };
        let mut lines: usize = bullet_texts
            .iter()
            .map(|t| {
                let n = t.trim().chars().count();
                if n == 0 {
                    1
                } else {
                    n.div_ceil(cpl)
                }
            })
            .sum();
        if subtitle_text.is_some() {
            lines += 1;
        }
        if lines > 14 {
            stats.overflow_slides += 1;
        }
    }

    // The bullet body, with an optional leading subtitle line on content layouts.
    let bullets_only: String = bullet_texts.iter().map(|t| bullet_para(t)).collect();
    let body_with_subtitle = || -> String {
        let mut b = String::new();
        if let Some(sub) = &subtitle_text {
            b.push_str(&subtitle_para(sub));
        }
        b.push_str(&bullets_only);
        b
    };

    match slide.layout.as_str() {
        "section" => {
            let title = heading
                .clone()
                .unwrap_or_else(|| bullet_texts.first().cloned().unwrap_or_default());
            shapes.push_str(&text_box(
                sid,
                "Title",
                MARGIN,
                SLIDE_H / 2 - 1_143_000,
                SLIDE_W - 2 * MARGIN,
                1_143_000,
                &title_para(&title),
            ));
            sid += 1;
            // Explicit subtitle wins; else the first bullet (positional fallback).
            let sub = subtitle_text.clone().or_else(|| bullet_texts.first().cloned());
            if let Some(sub) = sub {
                shapes.push_str(&text_box(
                    sid,
                    "Subtitle",
                    MARGIN,
                    SLIDE_H / 2 + 50_000,
                    SLIDE_W - 2 * MARGIN,
                    900_000,
                    &subtitle_para(&sub),
                ));
            }
        }
        "title-image" => {
            if let Some(h) = &heading {
                shapes.push_str(&title_box(sid, h));
                sid += 1;
            }
            let body_cx = (SLIDE_W - 2 * MARGIN) * 55 / 100;
            shapes.push_str(&text_box(
                sid,
                "Body",
                MARGIN,
                BODY_Y,
                body_cx,
                SLIDE_H - BODY_Y - MARGIN,
                &body_with_subtitle(),
            ));
            sid += 1;
            // Only the first image fits this layout; count any extras as dropped.
            if imgs.len() > 1 {
                stats.extra_images += imgs.len() - 1;
            }
            if let Some(img) = imgs.first() {
                match decode_image(&img.content) {
                    // Empty/cleared content = a remote fetch that failed earlier.
                    None => stats.fetch_failed += 1,
                    Some(bytes) => match image_ext(&bytes) {
                        None => stats.unsupported_format += 1,
                        Some((ext, _kind)) => {
                            *media_counter += 1;
                            let rid = "rId2".to_string(); // single image rel per slide
                            let file = format!("image{}.{}", media_counter, ext);
                            // Fit inside the right-hand box, preserving aspect ratio.
                            let box_x = MARGIN + body_cx + 400_050;
                            let box_y = BODY_Y;
                            let box_cx = SLIDE_W - MARGIN - box_x;
                            let box_cy = SLIDE_H - BODY_Y - MARGIN;
                            let (x, y, cx, cy) = fit(&bytes, box_x, box_y, box_cx, box_cy);
                            shapes.push_str(&picture(sid, &rid, x, y, cx, cy));
                            images.push(SlideImage { rid, file, bytes });
                        }
                    },
                }
            }
        }
        _ => {
            // "title-content": title (+ optional subtitle) + full-width bullets
            if let Some(h) = &heading {
                shapes.push_str(&title_box(sid, h));
                sid += 1;
            }
            shapes.push_str(&text_box(
                sid,
                "Body",
                MARGIN,
                BODY_Y,
                SLIDE_W - 2 * MARGIN,
                SLIDE_H - BODY_Y - MARGIN,
                &body_with_subtitle(),
            ));
        }
    }

    // Images on a layout that doesn't render them (a section/title-content slide
    // whose layout was overridden) are dropped — surface that rather than lose
    // them silently (A7).
    if slide.layout != "title-image" && !imgs.is_empty() {
        stats.extra_images += imgs.len();
    }

    let sp_tree = format!(
        r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}"#
    );
    (sp_tree, images)
}

// ----- shape / run builders -------------------------------------------------

fn esc(s: &str) -> String {
    // B1: strip characters the XML 1.0 `Char` production forbids BEFORE escaping.
    // C0 control characters (except tab/LF/CR) and the noncharacters U+FFFE/FFFF
    // are illegal inside `<a:t>`/`<dc:title>`; left in, PowerPoint reports the
    // .pptx as corrupt and offers "repair" — an unopenable file, produced
    // silently. Such characters are common in text pasted from PDFs. (Rust `char`
    // is always a Unicode scalar value, so surrogates can't occur here.)
    s.chars()
        .filter(|&c| {
            matches!(c,
                '\t' | '\n' | '\r'
                | '\u{20}'..='\u{D7FF}'
                | '\u{E000}'..='\u{FFFD}'
                | '\u{10000}'..='\u{10FFFF}')
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn title_para(text: &str) -> String {
    format!(
        r#"<a:p><a:pPr/><a:r><a:rPr lang="en-US" sz="2800" b="1" dirty="0"/><a:t>{}</a:t></a:r></a:p>"#,
        esc(text)
    )
}

fn subtitle_para(text: &str) -> String {
    format!(
        r#"<a:p><a:pPr/><a:r><a:rPr lang="en-US" sz="2000" dirty="0"><a:solidFill><a:schemeClr val="tx2"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>"#,
        esc(text)
    )
}

fn bullet_para(text: &str) -> String {
    // Blank paragraphs render as an empty bullet; collapse to a spacer instead.
    let t = text.trim();
    if t.is_empty() {
        return r#"<a:p><a:pPr/><a:endParaRPr lang="en-US"/></a:p>"#.to_string();
    }
    format!(
        r#"<a:p><a:pPr marL="285750" indent="-285750"><a:buFont typeface="Arial"/><a:buChar char="&#8226;"/></a:pPr><a:r><a:rPr lang="en-US" sz="1800" dirty="0"/><a:t>{}</a:t></a:r></a:p>"#,
        esc(t)
    )
}

fn title_box(id: u32, text: &str) -> String {
    text_box(
        id,
        "Title",
        MARGIN,
        365_760,
        SLIDE_W - 2 * MARGIN,
        1_000_000,
        &title_para(text),
    )
}

fn text_box(id: u32, name: &str, x: i64, y: i64, cx: i64, cy: i64, paras: &str) -> String {
    let body = if paras.is_empty() {
        r#"<a:p><a:endParaRPr lang="en-US"/></a:p>"#.to_string()
    } else {
        paras.to_string()
    };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap="square" rtlCol="0"><a:normAutofit/></a:bodyPr><a:lstStyle/>{body}</p:txBody></p:sp>"#,
        id = id,
        name = esc(name),
    )
}

fn picture(id: u32, rid: &str, x: i64, y: i64, cx: i64, cy: i64) -> String {
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Image {id}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
    )
}

// ----- images ---------------------------------------------------------------

/// Decode an image chunk's `content` (a `data:...;base64,` URL, or bare base64).
fn decode_image(content: &str) -> Option<Vec<u8>> {
    let b64 = match content.find("base64,") {
        Some(i) => &content[i + "base64,".len()..],
        None => content,
    };
    let b64 = b64.trim();
    if b64.is_empty() {
        return None; // e.g. a remote image whose fetch failed (content cleared)
    }
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

/// Detect file extension + content kind from magic bytes, limited to the raster
/// formats PowerPoint embeds reliably (PNG/JPEG/GIF/BMP). Returns `None` for
/// anything else (WEBP, SVG, unknown) so the caller skips it and warns, instead
/// of writing mismatched bytes under a `.png` name that opens as a broken image
/// (B5).
fn image_ext(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some(("png", "image/png"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpeg", "image/jpeg"))
    } else if bytes.starts_with(b"GIF8") {
        // GIF87a and GIF89a both begin "GIF8".
        Some(("gif", "image/gif"))
    } else if bytes.starts_with(&[0x42, 0x4D]) {
        Some(("bmp", "image/bmp"))
    } else {
        None
    }
}

/// Pixel dimensions for PNG / JPEG, used to preserve aspect ratio on export.
fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    // PNG: 8-byte sig, then IHDR with width@16 height@20 (big-endian).
    if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    // JPEG: walk segment markers to a Start-Of-Frame (SOFn).
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            // SOF0..SOF15 carry the frame size, excluding DHT/JPG/DAC/RST/markers.
            let is_sof = (0xC0..=0xCF).contains(&marker)
                && marker != 0xC4
                && marker != 0xC8
                && marker != 0xCC;
            if is_sof {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                if w > 0 && h > 0 {
                    return Some((w, h));
                }
                return None;
            }
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if len < 2 {
                break;
            }
            i += 2 + len;
        }
    }
    // GIF: logical-screen width@6 / height@8 (little-endian u16).
    if bytes.len() >= 10 && bytes.starts_with(b"GIF8") {
        let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        if w > 0 && h > 0 {
            return Some((w, h));
        }
    }
    // BMP: BITMAPINFOHEADER width@18 / height@22 (little-endian i32; height may
    // be negative for a top-down bitmap).
    if bytes.len() >= 26 && bytes.starts_with(&[0x42, 0x4D]) {
        let w = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
        // `unsigned_abs` (not `abs`) so a crafted height of i32::MIN doesn't
        // overflow/panic on attacker-controlled image bytes.
        let h = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]).unsigned_abs();
        if w > 0 && h > 0 {
            return Some((w as u32, h));
        }
    }
    None
}

/// Fit an image inside a box (EMU), preserving aspect ratio and centering it.
/// Returns `(x, y, cx, cy)`.
fn fit(bytes: &[u8], box_x: i64, box_y: i64, box_cx: i64, box_cy: i64) -> (i64, i64, i64, i64) {
    let (iw, ih) = image_size(bytes).unwrap_or((16, 9));
    let (iw, ih) = (iw as i64, ih as i64);
    // Compare aspect ratios via cross-multiplication (avoid float).
    let (cx, cy) = if iw * box_cy > ih * box_cx {
        (box_cx, box_cx * ih / iw) // width-bound
    } else {
        (box_cy * iw / ih, box_cy) // height-bound
    };
    let x = box_x + (box_cx - cx) / 2;
    let y = box_y + (box_cy - cy) / 2;
    (x, y, cx, cy)
}

// ----- per-slide XML --------------------------------------------------------

fn slide_xml(sp_tree: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree>{sp_tree}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    )
}

fn slide_rels(images: &[SlideImage]) -> String {
    let mut rels = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#,
    );
    for img in images {
        rels.push_str(&format!(
            r#"<Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{file}"/>"#,
            rid = img.rid,
            file = img.file
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
    )
}

// ----- fixed package parts --------------------------------------------------

fn content_types(n_slides: usize) -> String {
    let mut overrides = String::new();
    for i in 1..=n_slides {
        overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/><Default Extension="gif" ContentType="image/gif"/><Default Extension="bmp" ContentType="image/bmp"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>{overrides}<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#
    )
}

const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;

fn presentation_xml(n_slides: usize) -> String {
    let mut ids = String::new();
    for i in 0..n_slides {
        ids.push_str(&format!(
            r#"<p:sldId id="{sid}" r:id="rId{rid}"/>"#,
            sid = 256 + i,
            rid = 2 + i
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{w}" cy="{h}" type="screen16x9"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#,
        w = SLIDE_W,
        h = SLIDE_H
    )
}

fn presentation_rels(n_slides: usize) -> String {
    let mut rels = String::from(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    for i in 0..n_slides {
        rels.push_str(&format!(
            r#"<Relationship Id="rId{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#,
            rid = 2 + i,
            n = i + 1
        ));
    }
    rels.push_str(&format!(
        r#"<Relationship Id="rId{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#,
        rid = 2 + n_slides
    ));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
    )
}

const SLIDE_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#;

const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;

const SLIDE_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;

const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="aix"><a:themeElements><a:clrScheme name="aix"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F2933"/></a:dk2><a:lt2><a:srgbClr val="F4F5F7"/></a:lt2><a:accent1><a:srgbClr val="2563EB"/></a:accent1><a:accent2><a:srgbClr val="0F9D58"/></a:accent2><a:accent3><a:srgbClr val="F4B400"/></a:accent3><a:accent4><a:srgbClr val="DB4437"/></a:accent4><a:accent5><a:srgbClr val="9333EA"/></a:accent5><a:accent6><a:srgbClr val="FF6D00"/></a:accent6><a:hlink><a:srgbClr val="2563EB"/></a:hlink><a:folHlink><a:srgbClr val="9333EA"/></a:folHlink></a:clrScheme><a:fontScheme name="aix"><a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="aix"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#;

fn core_xml(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>{title}</dc:title><dc:creator>aixTextEditor</dc:creator><cp:lastModifiedBy>aixTextEditor</cp:lastModifiedBy></cp:coreProperties>"#,
        title = esc(title)
    )
}

fn app_xml(n_slides: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>aixTextEditor</Application><Slides>{n}</Slides><PresentationFormat>Widescreen</PresentationFormat></Properties>"#,
        n = n_slides
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::document_to_deck;
    use crate::models::{Chunk, Document};

    #[test]
    fn document_exports_to_valid_pptx_zip() {
        let mut doc = Document::new("Test Deck");
        doc.chunks.push(Chunk::new_heading(0, 1, "Section One"));
        doc.chunks.push(Chunk::new_text(1, "First bullet"));
        doc.chunks.push(Chunk::new_text(2, "Second & <special> \"quote\""));
        let deck = document_to_deck(&doc);
        let (bytes, warnings) = deck_to_pptx(&deck).expect("build pptx");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        if std::env::var("PPTX_DUMP").is_ok() {
            std::fs::write("/tmp/aix_real.pptx", &bytes).ok();
        }

        assert!(bytes.starts_with(b"PK"), "not a zip");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("open zip");
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        for req in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/slides/slide1.xml",
            "ppt/theme/theme1.xml",
            "ppt/slideMasters/slideMaster1.xml",
        ] {
            assert!(names.iter().any(|n| n == req), "missing part {req}");
        }
    }

    #[test]
    fn unresolved_image_is_reported_not_silently_dropped() {
        // An image chunk whose content is a remote URL (unresolved here, as a
        // failed fetch would leave it) must produce a visible warning.
        let mut doc = Document::new("D");
        doc.chunks.push(Chunk::new_heading(0, 1, "Has a picture"));
        doc.chunks.push(Chunk::new_text(1, "caption"));
        let mut img = Chunk::new_text(2, "https://example.com/x.png");
        img.metadata.chunk_type = crate::models::CHUNK_TYPE_IMAGE.to_string();
        doc.chunks.push(img);
        let deck = document_to_deck(&doc);
        let (_bytes, warnings) = deck_to_pptx(&deck).expect("build");
        assert!(
            warnings.iter().any(|w| w.contains("image")),
            "expected an image warning, got {warnings:?}"
        );
    }

    #[test]
    fn fit_preserves_aspect_within_box() {
        // 480x270 (16:9) PNG header bytes are enough for image_size.
        let mut png = vec![0x89, b'P', b'N', b'G', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        png.extend_from_slice(&480u32.to_be_bytes());
        png.extend_from_slice(&270u32.to_be_bytes());
        let (_, _, cx, cy) = fit(&png, 0, 0, 4_000_000, 4_000_000);
        // width-bound: cy/cx should be ~270/480
        assert!((cx as f64 * 270.0 / 480.0 - cy as f64).abs() < 2.0);
    }

    // ----- B1: control-character stripping -----

    #[test]
    fn esc_strips_c0_controls_keeps_tab_nl_cr() {
        let input = "a\u{0}b\u{1}c\u{8}\u{B}\u{C}\u{1F}d\te\nf\rg";
        assert_eq!(esc(input), "abcd\te\nf\rg");
    }

    #[test]
    fn esc_drops_noncharacters_keeps_unicode() {
        // U+FFFE/U+FFFF are illegal in XML; astral chars (U+1F600) must survive.
        assert_eq!(esc("x\u{FFFE}y\u{FFFF}z日本😀"), "xyz日本😀");
    }

    #[test]
    fn esc_escapes_xml_entities() {
        assert_eq!(esc("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }

    #[test]
    fn export_with_control_chars_produces_legal_xml() {
        // A document whose title and body carry C0 control chars must still
        // produce slide/core XML with no illegal bytes (otherwise PowerPoint
        // reports the file as corrupt).
        let mut doc = Document::new("Title\u{7}with\u{1}bell");
        doc.chunks.push(Chunk::new_heading(0, 1, "Head\u{8}ing"));
        doc.chunks.push(Chunk::new_text(1, "body\u{1F}text\u{0}here"));
        let deck = document_to_deck(&doc);
        let (bytes, _w) = deck_to_pptx(&deck).expect("build");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        for part in ["ppt/slides/slide1.xml", "docProps/core.xml"] {
            use std::io::Read;
            let mut s = String::new();
            zip.by_name(part).unwrap().read_to_string(&mut s).unwrap();
            assert!(
                !s.bytes()
                    .any(|b| b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r'),
                "{part} still contains an illegal control byte"
            );
        }
    }

    // ----- B5: image format detection -----

    #[test]
    fn image_ext_recognizes_embeddable_formats() {
        assert_eq!(image_ext(&[0x89, 0x50, 0x4E, 0x47, 1, 2]), Some(("png", "image/png")));
        assert_eq!(image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(("jpeg", "image/jpeg")));
        assert_eq!(image_ext(b"GIF89a..."), Some(("gif", "image/gif")));
        assert_eq!(image_ext(b"GIF87a..."), Some(("gif", "image/gif")));
        assert_eq!(image_ext(&[0x42, 0x4D, 1, 2]), Some(("bmp", "image/bmp")));
    }

    #[test]
    fn image_ext_skips_unsupported() {
        // WEBP (RIFF....WEBP), SVG-ish text, and junk are not embeddable.
        let webp = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50];
        assert_eq!(image_ext(&webp), None);
        assert_eq!(image_ext(b"<svg xmlns=..."), None);
        assert_eq!(image_ext(&[0, 1, 2, 3]), None);
    }

    #[test]
    fn image_size_parses_gif_and_bmp() {
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&4u16.to_le_bytes()); // width
        gif.extend_from_slice(&2u16.to_le_bytes()); // height
        assert_eq!(image_size(&gif), Some((4, 2)));

        let mut bmp = vec![0x42u8, 0x4D];
        bmp.extend_from_slice(&[0u8; 16]); // up to offset 18
        bmp.extend_from_slice(&4i32.to_le_bytes()); // width @18
        bmp.extend_from_slice(&2i32.to_le_bytes()); // height @22
        assert_eq!(image_size(&bmp), Some((4, 2)));
    }

    // ----- B5 / A7: embed + warning integration -----

    fn data_url(mime: &str, bytes: &[u8]) -> String {
        use base64::Engine;
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    fn image_chunk(order: u32, content: String) -> Chunk {
        let mut c = Chunk::new_text(order, content);
        c.metadata.chunk_type = crate::models::CHUNK_TYPE_IMAGE.to_string();
        c
    }

    #[test]
    fn gif_image_embedded_with_gif_part() {
        let mut doc = Document::new("D");
        doc.chunks.push(Chunk::new_heading(0, 1, "Pic"));
        doc.chunks
            .push(image_chunk(1, data_url("image/gif", b"GIF89a\x04\x00\x02\x00\x80\x00\x00")));
        let deck = document_to_deck(&doc);
        let (bytes, warnings) = deck_to_pptx(&deck).expect("build");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let parts: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(parts.iter().any(|n| n == "ppt/media/image1.gif"), "no gif media part: {parts:?}");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn webp_image_warns_and_is_not_embedded() {
        let webp = [0x52u8, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0, 0];
        let mut doc = Document::new("D");
        doc.chunks.push(Chunk::new_heading(0, 1, "Pic"));
        doc.chunks.push(image_chunk(1, data_url("image/webp", &webp)));
        let deck = document_to_deck(&doc);
        let (bytes, warnings) = deck_to_pptx(&deck).expect("build");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let parts: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().to_string()).collect();
        assert!(!parts.iter().any(|n| n.starts_with("ppt/media/")), "webp must not be embedded");
        assert!(warnings.iter().any(|w| w.contains("format")), "expected a format warning: {warnings:?}");
    }

    #[test]
    fn two_images_warns_extra_and_embeds_one() {
        let mut doc = Document::new("D");
        doc.chunks.push(Chunk::new_heading(0, 1, "Pic"));
        doc.chunks.push(image_chunk(1, data_url("image/png", b"\x89PNG\r\n\x1a\n")));
        doc.chunks.push(image_chunk(2, data_url("image/png", b"\x89PNG\r\n\x1a\n")));
        let deck = document_to_deck(&doc);
        let (bytes, warnings) = deck_to_pptx(&deck).expect("build");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let media = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .filter(|n| n.starts_with("ppt/media/"))
            .count();
        assert_eq!(media, 1, "exactly one image should embed");
        assert!(warnings.iter().any(|w| w.contains("extra")), "expected an extra-image warning: {warnings:?}");
    }

    fn slide1_xml(doc: &Document) -> String {
        use std::io::Read;
        let deck = document_to_deck(doc);
        let (bytes, _w) = deck_to_pptx(&deck).expect("build");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let mut xml = String::new();
        zip.by_name("ppt/slides/slide1.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        xml
    }

    #[test]
    fn explicit_subtitle_renders_on_section_slide() {
        let mut doc = Document::new("D");
        let mut h = Chunk::new_heading(0, 1, "Cover");
        h.metadata.layout = Some("section".to_string());
        doc.chunks.push(h);
        let mut sub = Chunk::new_text(1, "The subtitle");
        sub.metadata.subtitle = true;
        doc.chunks.push(sub);
        let xml = slide1_xml(&doc);
        assert!(xml.contains("Cover"), "title missing: {xml}");
        assert!(xml.contains("The subtitle"), "subtitle missing: {xml}");
    }

    #[test]
    fn detached_slide_body_replaces_the_prose_bullets() {
        let mut doc = Document::new("D");
        let mut h = Chunk::new_heading(0, 1, "Topic");
        h.metadata.slide_body = Some(vec!["Summary one".into(), "Summary two".into()]);
        doc.chunks.push(h);
        doc.chunks.push(Chunk::new_text(1, "original prose paragraph"));
        let xml = slide1_xml(&doc);
        assert!(xml.contains("Summary one") && xml.contains("Summary two"), "slideBody missing: {xml}");
        assert!(!xml.contains("original prose"), "prose should be ignored when detached: {xml}");
    }

    #[test]
    fn long_bullets_warn_overflow_but_short_deck_does_not() {
        let mut doc = Document::new("D");
        doc.chunks.push(Chunk::new_heading(0, 1, "Dense"));
        for i in 0..20 {
            doc.chunks.push(Chunk::new_text(i + 1, "x".repeat(100)));
        }
        let deck = document_to_deck(&doc);
        let (_b, warnings) = deck_to_pptx(&deck).expect("build");
        assert!(warnings.iter().any(|w| w.contains("cut off")), "expected an overflow warning: {warnings:?}");
    }
}
