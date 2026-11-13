#!/usr/bin/env bash
set -euo pipefail
binary=$1
out=$2
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' crates/camera-lab/Cargo.toml)
test -n "$version"
mkdir -p "$out"
out=$(cd "$out" && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
case "$(uname -s):$(uname -m)" in
  Darwin:arm64)
    name="kohdalab-camera-$version-macos-arm64"
    app="$scratch/KohdaLab Camera.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    install -m 755 "$binary" "$app/Contents/MacOS/camera-lab"
    cp -R LICENSE THIRD_PARTY_NOTICES.md third-party-licenses "$app/Contents/Resources/"
    cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.kohdalab.camera</string>
<key>CFBundleName</key><string>KohdaLab Camera</string>
<key>CFBundleExecutable</key><string>camera-lab</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
</dict></plist>
PLIST
    plutil -lint "$app/Contents/Info.plist"
    codesign --force --sign - "$app"
    codesign --verify --strict "$app"
    ditto -c -k --keepParent "$app" "$out/$name-portable.zip"
    ln -s /Applications "$scratch/Applications"
    hdiutil create -fs HFS+ -format UDZO -volname "KohdaLab Camera" -srcfolder "$scratch" "$out/$name.dmg"
    ;;
  Linux:x86_64)
    name="kohdalab-camera-$version-linux-x64"
    root="$scratch/$name"
    mkdir -p "$root"
    install -m 755 "$binary" "$root/kohdalab-camera"
    cp -R LICENSE THIRD_PARTY_NOTICES.md third-party-licenses "$root/"
    tar -C "$scratch" -czf "$out/$name-portable.tar.gz" "$name"
    pkg="$scratch/deb"
    mkdir -p "$pkg/DEBIAN" "$pkg/usr/bin" "$pkg/usr/share/doc/kohdalab-camera" "$pkg/usr/share/applications"
    install -m 755 "$binary" "$pkg/usr/bin/kohdalab-camera"
    cp -R LICENSE THIRD_PARTY_NOTICES.md third-party-licenses "$pkg/usr/share/doc/kohdalab-camera/"
    cat > "$pkg/DEBIAN/control" <<CONTROL
Package: kohdalab-camera
Version: $version
Architecture: amd64
Maintainer: KohdaLab
Depends: libc6 (>= 2.35), libgcc-s1, libgl1, libx11-6, libxkbcommon0, libxkbcommon-x11-0, libegl1, libgl1-mesa-dri, libwayland-client0
Description: KohdaLab Camera desktop application (Simulation)
CONTROL
    cat > "$pkg/usr/share/applications/kohdalab-camera.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=KohdaLab Camera
Exec=kohdalab-camera
Terminal=false
Categories=Science;
DESKTOP
    dpkg-deb --root-owner-group --build "$pkg" "$out/$name.deb"
    ;;
  *) echo 'Unsupported package host' >&2; exit 1 ;;
esac
