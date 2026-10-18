#!/bin/sh
# Makes claudius.app, one universal app for Apple silicon and Intel Macs, from the two macOS release archives, and the
# disk image people install it from: claudius-<version>-universal-apple-darwin.dmg, which holds the app and a link to
# Applications to drag it onto. It uses only the tools macOS comes with, so the release job that signs runs no build.
#
#   scripts/package/macos.sh <aarch64-apple-darwin archive> <x86_64-apple-darwin archive> <out dir>
#
# Signing, when the environment has it:
#   MACOS_SIGN_IDENTITY   a Developer ID Application identity in the keychain. The app and the disk image are signed
#                         with it, with the hardened runtime. Without it they are signed ad hoc, and macOS asks before
#                         it opens the app the first time.
#   MACOS_NOTARY_KEY, MACOS_NOTARY_KEY_ID, MACOS_NOTARY_ISSUER
#                         an App Store Connect API key: the text of its .p8 file, its key id and the issuer id. Apple
#                         notarizes the app and the disk image, and their tickets are stapled to them, so Gatekeeper
#                         opens them without asking, offline too.

set -eu

if [ $# -ne 3 ]; then
    echo "usage: $0 <aarch64-apple-darwin archive> <x86_64-apple-darwin archive> <out dir>" >&2
    exit 2
fi
arm=$1
intel=$2
out=$3
root=$(cd "$(dirname "$0")/../.." && pwd)

name=$(basename "$arm" .tar.gz)
version=${name#claudius-}
version=${version%-aarch64-apple-darwin}
if [ "$(basename "$intel")" != "claudius-$version-x86_64-apple-darwin.tar.gz" ]; then
    echo "macos.sh: $intel is not the x86_64 archive of claudius $version" >&2
    exit 2
fi
# CFBundleVersion takes numbers only, so a pre-release (0.5.0-rc.1) is 0.5.0 to macOS
bundle_version=${version%%-*}
if [ -n "${MACOS_NOTARY_KEY:-}" ] && [ -z "${MACOS_SIGN_IDENTITY:-}" ]; then
    echo "macos.sh: notarizing needs a signed app; set MACOS_SIGN_IDENTITY too" >&2
    exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/arm" "$tmp/intel"
tar -xzf "$arm" -C "$tmp/arm" --strip-components 1
tar -xzf "$intel" -C "$tmp/intel" --strip-components 1

app=$tmp/dmg/claudius.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for binary in claudius claudius-packs; do
    lipo -create "$tmp/arm/bin/$binary" "$tmp/intel/bin/$binary" -output "$app/Contents/MacOS/$binary"
done
cp -R "$tmp/arm/share/claudius/web" "$app/Contents/Resources/web"
cp "$root/packaging/icons/claudius.icns" "$app/Contents/Resources/"
for doc in LICENSE NOTICE; do
    if [ -f "$tmp/arm/$doc" ]; then cp "$tmp/arm/$doc" "$app/Contents/Resources/"; fi
done
sed "s/{{version}}/$bundle_version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
plutil -lint -s "$app/Contents/Info.plist"

sign() {
    if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
        codesign --force --timestamp --options runtime --sign "$MACOS_SIGN_IDENTITY" "$@"
    else
        codesign --force --sign - "$@"
    fi
}

if [ -n "${MACOS_NOTARY_KEY:-}" ]; then
    key=$tmp/notary.p8
    (umask 077 && printf '%s\n' "$MACOS_NOTARY_KEY" > "$key")
fi
notarize() {
    result=$(xcrun notarytool submit "$1" --key "$key" --key-id "$MACOS_NOTARY_KEY_ID" \
        --issuer "$MACOS_NOTARY_ISSUER" --wait --output-format json)
    status=$(printf '%s' "$result" | plutil -extract status raw -o - -)
    if [ "$status" != Accepted ]; then
        id=$(printf '%s' "$result" | plutil -extract id raw -o - -)
        echo "macos.sh: Apple did not notarize $(basename "$1") ($status); its log:" >&2
        xcrun notarytool log "$id" --key "$key" --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER" >&2
        exit 1
    fi
}

# the helper program first, then the bundle, which seals the main program, the Info.plist and the resources
sign "$app/Contents/MacOS/claudius-packs"
sign "$app"
codesign --verify --strict --deep "$app"
if [ -n "${MACOS_NOTARY_KEY:-}" ]; then
    ditto -c -k --keepParent "$app" "$tmp/claudius.zip"
    notarize "$tmp/claudius.zip"
    xcrun stapler staple -q "$app"
fi

ln -s /Applications "$tmp/dmg/Applications"
mkdir -p "$out"
dmg=$out/claudius-$version-universal-apple-darwin.dmg
rm -f "$dmg"
hdiutil create -quiet -volname "claudius $version" -srcfolder "$tmp/dmg" -fs HFS+ -format UDZO "$dmg"
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$dmg"
fi
if [ -n "${MACOS_NOTARY_KEY:-}" ]; then
    notarize "$dmg"
    xcrun stapler staple -q "$dmg"
fi
(cd "$out" && shasum -a 256 "$(basename "$dmg")" > "$(basename "$dmg").sha256")
echo "macos.sh: $dmg"
