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
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };
    for slide in &mut deck.slides {
        for chunk in &mut slide.chunks {
            if !chunk.is_image() {
                continue;
            }
            let url = chunk.content.trim();
            if url.starts_with("http://") || url.starts_with("https://") {
                chunk.content = match fetch_as_data_url(&client, url).await {
                    Ok(data_url) => data_url,
                    Err(_) => String::new(), // cleared → counted as a dropped image
                };
            }
        }
    }
}

async fn fetch_as_data_url(client: &reqwest::Client, url: &str) -> AppResult<String> {
    let bytes = client.get(url).send().await?.bytes().await?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    // The mime here is cosmetic — `image_ext` re-sniffs the magic bytes on write.
    Ok(format!("data:image/png;base64,{b64}"))
}

/// Build the `.pptx` bytes for a deck, plus any non-fatal warnings. Call
/// `resolve_remote_images` first so remote image URLs become embeddable.
pub fn deck_to_pptx(deck: &Deck) -> AppResult<(Vec<u8>, Vec<String>)> {
    let n = deck.slides.len();

    // Tally what the layout can't carry yet, so the caller can surface it.
    let total_images: usize = deck
        .slides
        .iter()
        .flat_map(|s| &s.chunks)
        .filter(|c| c.is_image())
        .count();
    let diagrams: usize = deck
        .slides
        .iter()
        .flat_map(|s| &s.chunks)
        .filter(|c| c.is_diagram())
        .count();
    let mut embedded = 0usize;
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
        let (sp_tree, images) = build_slide(slide, &mut media_counter);
        embedded += images.len();
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
    let dropped_images = total_images.saturating_sub(embedded);
    if dropped_images > 0 {
        warnings.push(format!(
            "{dropped_images} image(s) couldn't be embedded (a remote image may have failed to download, or only the first image per slide is kept for now)."
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

fn is_text(c: &Chunk) -> bool {
    c.metadata.chunk_type == CHUNK_TYPE_TEXT
}

fn build_slide(slide: &Slide, media_counter: &mut usize) -> (String, Vec<SlideImage>) {
    let mut shapes = String::new();
    let mut images = Vec::new();
    let mut sid: u32 = 2; // shape id 1 is the group

    let heading = slide
        .chunks
        .iter()
        .find(|c| c.is_heading())
        .map(|c| c.content.clone());
    let texts: Vec<&Chunk> = slide.chunks.iter().filter(|c| is_text(c)).collect();
    let image = slide.chunks.iter().find(|c| c.is_image());

    match slide.layout.as_str() {
        "section" => {
            let title = heading.unwrap_or_else(|| slide_first_text(&texts));
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
            if let Some(sub) = texts.first() {
                shapes.push_str(&text_box(
                    sid,
                    "Subtitle",
                    MARGIN,
                    SLIDE_H / 2 + 50_000,
                    SLIDE_W - 2 * MARGIN,
                    900_000,
                    &subtitle_para(&sub.content),
                ));
            }
        }
        "title-image" => {
            if let Some(h) = &heading {
                shapes.push_str(&title_box(sid, h));
                sid += 1;
            }
            let body_cx = (SLIDE_W - 2 * MARGIN) * 55 / 100;
            let bullets: String = texts.iter().map(|c| bullet_para(&c.content)).collect();
            shapes.push_str(&text_box(
                sid,
                "Body",
                MARGIN,
                BODY_Y,
                body_cx,
                SLIDE_H - BODY_Y - MARGIN,
                &bullets,
            ));
            sid += 1;
            if let Some(img) = image {
                if let Some(bytes) = decode_image(&img.content) {
                    *media_counter += 1;
                    let (ext, content_kind) = image_ext(&bytes);
                    let rid = "rId2".to_string(); // single image rel per slide
                    let file = format!("image{}.{}", media_counter, ext);
                    // Fit inside the right-hand box, preserving aspect ratio.
                    let box_x = MARGIN + body_cx + 400_050;
                    let box_y = BODY_Y;
                    let box_cx = SLIDE_W - MARGIN - box_x;
                    let box_cy = SLIDE_H - BODY_Y - MARGIN;
                    let (x, y, cx, cy) = fit(&bytes, box_x, box_y, box_cx, box_cy);
                    let _ = content_kind;
                    shapes.push_str(&picture(sid, &rid, x, y, cx, cy));
                    images.push(SlideImage { rid, file, bytes });
                }
            }
        }
        _ => {
            // "title-content": title + full-width bullets
            if let Some(h) = &heading {
                shapes.push_str(&title_box(sid, h));
                sid += 1;
            }
            let bullets: String = texts.iter().map(|c| bullet_para(&c.content)).collect();
            shapes.push_str(&text_box(
                sid,
                "Body",
                MARGIN,
                BODY_Y,
                SLIDE_W - 2 * MARGIN,
                SLIDE_H - BODY_Y - MARGIN,
                &bullets,
            ));
        }
    }

    let sp_tree = format!(
        r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}"#
    );
    (sp_tree, images)
}

fn slide_first_text(texts: &[&Chunk]) -> String {
    texts.first().map(|c| c.content.clone()).unwrap_or_default()
}

// ----- shape / run builders -------------------------------------------------

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
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

/// Detect file extension + content kind from magic bytes (defaults to png).
fn image_ext(bytes: &[u8]) -> (&'static str, &'static str) {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("jpeg", "image/jpeg")
    } else {
        ("png", "image/png")
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
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>{overrides}<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#
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
}
