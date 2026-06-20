# Homebrew formula — builds aixTextEditor from source on the user's Mac.
# A local source build carries no `com.apple.quarantine` flag, so the app opens
# with no Gatekeeper / notarization prompt, and it targets the host CPU
# (Apple Silicon or Intel) automatically.
#
# This formula lives in the app's own public repo (kumeS/aixTextEditor) under
# Formula/. Two ways to install it:
#
#   A) Single repo — no separate tap repo needed (two commands):
#        brew tap kumeS/tap https://github.com/kumeS/aixTextEditor
#        brew install kumeS/tap/aixtexteditor
#
#   B) One command for anyone — needs a tap repo literally named
#      kumeS/homebrew-tap with this file in its Formula/ dir:
#        # then: brew install kumeS/tap/aixtexteditor   (no prior `brew tap`)
#
# After tagging a new release, refresh `sha256`:
#   curl -sL https://github.com/kumeS/aixTextEditor/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256
class Aixtexteditor < Formula
  desc "LLM-augmented chunk-based editor for academic papers and reports"
  homepage "https://github.com/kumeS/aixTextEditor"
  url "https://github.com/kumeS/aixTextEditor/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "107d18020b28bb4f89e79136a9382ef074273bdbba3dd27747ddda48ee501810"
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
