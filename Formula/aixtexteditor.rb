# Homebrew formula — builds aixTextEditor from source on the user's Mac.
#
# This is the RECOMMENDED install path. A local source build carries no
# `com.apple.quarantine` flag, so the app opens with no Gatekeeper / notarization
# prompt, and it targets the host CPU (Apple Silicon or Intel) automatically.
#
# Ship it from a tap repo named `kumeS/homebrew-tap`:
#     cp Formula/aixtexteditor.rb <homebrew-tap>/Formula/aixtexteditor.rb
# Users then run:
#     brew install kumeS/tap/aixtexteditor
#
# The source must be publicly fetchable for this to work on other machines:
#   1. Make the GitHub repo public and push a tag (e.g. v1.0.0).
#   2. Compute the source-tarball sha256:
#        curl -sL https://github.com/kumeS/aixTextEditor/archive/refs/tags/v1.0.0.tar.gz | shasum -a 256
#   3. Paste the value into `sha256` below (replacing the placeholder).
#
# Verified locally: `npm ci` + `npx tauri build` run inside Homebrew's build
# sandbox (network is permitted there by default) and produce a runnable
# aixTextEditor.app in ~2 minutes on Apple Silicon.
class Aixtexteditor < Formula
  desc "LLM-augmented chunk-based editor for academic papers and reports"
  homepage "https://github.com/kumeS/aixTextEditor"
  url "https://github.com/kumeS/aixTextEditor/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "REPLACE_WITH_TARBALL_SHA256" # see header: compute after the repo is public
  license "Artistic-2.0"
  head "https://github.com/kumeS/aixTextEditor.git", branch: "main"

  depends_on "node" => :build
  depends_on "rust" => :build
  depends_on :macos

  def install
    # Keep package-manager caches inside the sandboxed build dir.
    ENV["CARGO_HOME"] = buildpath/".cargo"
    ENV["npm_config_cache"] = buildpath/".npm"

    system "npm", "ci"
    # Build only the .app — the .dmg step shells out to Finder/AppleScript,
    # which is unavailable in Homebrew's non-interactive sandbox.
    system "npx", "tauri", "build", "--bundles", "app"

    prefix.install "src-tauri/target/release/bundle/macos/aixTextEditor.app"

    # Convenience CLI launcher: `aixtexteditor` opens the app.
    (bin/"aixtexteditor").write <<~SH
      #!/bin/bash
      exec open -a "#{opt_prefix}/aixTextEditor.app" "$@"
    SH
  end

  def caveats
    <<~EOS
      aixTextEditor was built from source — it has no quarantine flag and opens
      without any Gatekeeper / notarization prompt.

      Launch it:
        aixtexteditor
      …or add it to /Applications:
        ln -sfn #{opt_prefix}/aixTextEditor.app /Applications/aixTextEditor.app

      On first run, open Settings (gear icon, or Cmd-,) and paste your OpenRouter
      API key. It is stored in the macOS keychain, never on disk in plaintext.
    EOS
  end

  test do
    assert_path_exists prefix/"aixTextEditor.app/Contents/MacOS/aix-text-editor"
  end
end
