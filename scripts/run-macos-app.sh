#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app_name="MoraNote"
bundle_dir="$repo_dir/target/debug/$app_name.app"
legacy_bundle_dir="$repo_dir/target/debug/Markdown Studio.app"
contents_dir="$bundle_dir/Contents"
macos_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"
binary_src="$repo_dir/target/debug/moranote"
binary_dst="$macos_dir/MoraNote"
icon_path="$repo_dir/assets/app-icons/moranote.icns"
iconset_path="$repo_dir/assets/app-icons/MoraNote.iconset"
icon_name="MoraNote.icns"

cd "$repo_dir"
cargo build

if [[ ! -f "$icon_path" ]]; then
  cargo run --example generate_icon
  iconutil -c icns "$iconset_path" -o "$icon_path"
fi

rm -rf "$bundle_dir"
rm -rf "$legacy_bundle_dir"
mkdir -p "$macos_dir" "$resources_dir"
cp "$binary_src" "$binary_dst"
chmod +x "$binary_dst"
mkdir -p "$resources_dir/assets/themes"
cp -R "$repo_dir/assets/themes/morandigarden" "$resources_dir/assets/themes/"
cp "$icon_path" "$resources_dir/$icon_name"

cat > "$contents_dir/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>zh_CN</string>
  <key>CFBundleDisplayName</key>
  <string>MoraNote</string>
  <key>CFBundleExecutable</key>
  <string>MoraNote</string>
  <key>CFBundleIdentifier</key>
  <string>local.moranote</string>
  <key>CFBundleIconFile</key>
  <string>MoraNote</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>MoraNote</string>
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
  <string>0.1.0</string>
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

lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$lsregister" ]]; then
  "$lsregister" -f "$bundle_dir" >/dev/null 2>&1 || true
fi

open -n "$bundle_dir"
echo "Started $bundle_dir"
