# Homebrew Cask for aixTextEditor.
#
# TEMPLATE — before this can be installed via `brew install`, you must:
#   1. Build a release bundle:  npm run tauri build   (produces a .dmg)
#   2. Upload the .dmg to a GitHub Release tagged v<version>.
#   3. Compute its checksum:    shasum -a 256 "aixTextEditor_<version>_aarch64.dmg"
#      and paste it into `sha256` below (replace :no_check).
#   4. Host this cask in a tap, e.g.  kumeS/homebrew-tap, then:
#         brew tap kumeS/tap
#         brew install --cask aix-text-editor
#
# NOTE: an unsigned / un-notarized .app is quarantined by Gatekeeper. Either
# sign + notarize the build, or users must run:
#   xattr -dr com.apple.quarantine "/Applications/aixTextEditor.app"
cask "aix-text-editor" do
  version "1.1.0"
  sha256 :no_check # replace with the real sha256 of the released .dmg

  url "https://github.com/kumeS/aixTextEditor/releases/download/v#{version}/aixTextEditor_#{version}_aarch64.dmg"
  name "aixTextEditor"
  desc "LLM-augmented, chunk-based editor for academic papers and reports"
  homepage "https://github.com/kumeS/aixTextEditor"

  depends_on macos: :big_sur

  app "aixTextEditor.app"

  zap trash: [
    "~/Library/Application Support/com.aix.texteditor",
    "~/Library/Preferences/com.aix.texteditor.plist",
    "~/Library/Saved Application State/com.aix.texteditor.savedState",
  ]
end
