#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app_name="MoraNote"
bundle_id="local.moranote"
binary_name="moranote"
executable_name="MoraNote"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo_dir/Cargo.toml" | head -n 1)"
arch="$(uname -m)"

dist_dir="$repo_dir/dist"
work_dir="$repo_dir/target/dmg"
bundle_dir="$dist_dir/$app_name.app"
contents_dir="$bundle_dir/Contents"
macos_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"
dmg_root="$work_dir/root"
dmg_path="$dist_dir/$app_name-$version-$arch.dmg"
icon_path="$repo_dir/assets/app-icons/moranote.icns"
iconset_path="$repo_dir/assets/app-icons/MoraNote.iconset"
icon_name="MoraNote.icns"

cd "$repo_dir"

if [[ ! -f "$icon_path" ]]; then
  cargo run --example generate_icon
  iconutil -c icns "$iconset_path" -o "$icon_path"
fi

cargo build --release

rm -rf "$bundle_dir" "$work_dir"
mkdir -p "$macos_dir" "$resources_dir/assets/themes" "$dmg_root"

ditto "$repo_dir/target/release/$binary_name" "$macos_dir/$executable_name"
chmod +x "$macos_dir/$executable_name"
strip "$macos_dir/$executable_name" || true

ditto "$repo_dir/assets/themes/morandigarden" "$resources_dir/assets/themes/morandigarden"
ditto "$icon_path" "$resources_dir/$icon_name"

cat > "$contents_dir/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>zh_CN</string>
  <key>CFBundleDisplayName</key>
  <string>$app_name</string>
  <key>CFBundleExecutable</key>
  <string>$executable_name</string>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleIconFile</key>
  <string>MoraNote</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$app_name</string>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>md</string>
      </array>
      <key>CFBundleTypeIconFile</key>
      <string>MoraNote</string>
      <key>CFBundleTypeName</key>
      <string>Markdown Document</string>
      <key>CFBundleTypeRole</key>
      <string>Editor</string>
      <key>LSHandlerRank</key>
      <string>Owner</string>
      <key>LSItemContentTypes</key>
      <array>
        <string>net.daringfireball.markdown</string>
        <string>public.plain-text</string>
      </array>
    </dict>
  </array>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSSupportsOpeningDocumentsInPlace</key>
  <true/>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>UTImportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeConformsTo</key>
      <array>
        <string>public.plain-text</string>
        <string>public.text</string>
      </array>
      <key>UTTypeDescription</key>
      <string>Markdown Document</string>
      <key>UTTypeIdentifier</key>
      <string>net.daringfireball.markdown</string>
      <key>UTTypeTagSpecification</key>
      <dict>
        <key>public.filename-extension</key>
        <array>
          <string>md</string>
        </array>
        <key>public.mime-type</key>
        <array>
          <string>text/markdown</string>
          <string>text/x-markdown</string>
        </array>
      </dict>
    </dict>
  </array>
</dict>
</plist>
PLIST

if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$bundle_dir" >/dev/null
fi

ditto "$bundle_dir" "$dmg_root/$app_name.app"
ln -s /Applications "$dmg_root/Applications"

rm -f "$dmg_path"
hdiutil create \
  -volname "$app_name" \
  -srcfolder "$dmg_root" \
  -format UDZO \
  -imagekey zlib-level=9 \
  "$dmg_path" >/dev/null

hdiutil verify "$dmg_path" >/dev/null

echo "Built $dmg_path"
du -sh "$bundle_dir" "$dmg_path"
