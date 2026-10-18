#!/bin/sh
# Makes the Linux packages from a release archive (the build with the studio window): a .deb for Debian and Ubuntu,
# an .rpm for Fedora and its relatives, and an AppImage that runs anywhere with glibc 2.38 or newer. The packages put
# the programs in /usr/bin, the web studio in /usr/share/claudius/web, and the studio in the desktop's app menu.
#
#   scripts/package/linux.sh <claudius-...-unknown-linux-gnu.tar.gz> <out dir>
#
# Needs dpkg-deb, rpmbuild and desktop-file-validate (on Ubuntu: apt install rpm desktop-file-utils). The AppImage
# tool and runtime are downloaded at pinned versions and checked against their SHA-256.

set -eu

if [ $# -ne 2 ]; then
    echo "usage: $0 <claudius-...-unknown-linux-gnu.tar.gz> <out dir>" >&2
    exit 2
fi
archive=$1
out=$2
root=$(cd "$(dirname "$0")/../.." && pwd)

name=$(basename "$archive" .tar.gz)
case $name in
    *-x86_64-unknown-linux-gnu) arch=x86_64 deb_arch=amd64 ;;
    *-aarch64-unknown-linux-gnu) arch=aarch64 deb_arch=arm64 ;;
    *) echo "linux.sh: $archive is not a Linux archive of claudius" >&2; exit 2 ;;
esac
version=${name#claudius-}
version=${version%-"$arch"-unknown-linux-gnu}
# a pre-release, 0.5.0-rc.1, sorts before 0.5.0 as 0.5.0~rc.1, in dpkg and rpm alike
package_version=$(printf '%s' "$version" | sed 's/-/~/')

appimagetool_version=1.9.1
runtime_version=20251108
case $arch in
    x86_64)
        appimagetool_sha256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
        runtime_sha256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
        ;;
    aarch64)
        appimagetool_sha256=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
        runtime_sha256=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444
        ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
tar -xzf "$archive" -C "$tmp"
unpacked=$tmp/$name
[ -x "$unpacked/bin/claudius" ] || { echo "linux.sh: $archive has no bin/claudius" >&2; exit 1; }
desktop=$root/packaging/linux/claudius-studio.desktop
desktop-file-validate "$desktop"

# the files as installed, under /usr
stage=$tmp/stage
mkdir -p "$stage/usr/bin" "$stage/usr/share/claudius" "$stage/usr/share/applications" \
    "$stage/usr/share/icons/hicolor/scalable/apps" "$stage/usr/share/icons/hicolor/512x512/apps" \
    "$stage/usr/share/doc/claudius"
cp "$unpacked/bin/claudius" "$unpacked/bin/claudius-packs" "$stage/usr/bin/"
cp -R "$unpacked/share/claudius/web" "$stage/usr/share/claudius/web"
cp "$desktop" "$stage/usr/share/applications/"
cp "$root/packaging/icons/claudius.svg" "$stage/usr/share/icons/hicolor/scalable/apps/claudius.svg"
cp "$root/packaging/icons/claudius-512.png" "$stage/usr/share/icons/hicolor/512x512/apps/claudius.png"
for doc in README.md CHANGELOG.md LICENSE NOTICE; do
    if [ -f "$unpacked/$doc" ]; then cp "$unpacked/$doc" "$stage/usr/share/doc/claudius/"; fi
done

summary="A beatmaker that makes songs the way a producer does"
description="claudius picks sounds, writes drums, bass, chords and melodies with music theory, arranges them into a
song with builds and drops, mixes and masters it, listens to the result, and changes what it does not like. It
comes with a studio, a script language, a command line, and MCP tools for assistants."
mkdir -p "$out"

# Debian and Ubuntu
deb=$tmp/deb
cp -R "$stage" "$deb"
mkdir "$deb/DEBIAN"
{
    echo "Package: claudius"
    echo "Version: $package_version"
    echo "Architecture: $deb_arch"
    echo "Maintainer: The claudius authors <github@devlato.com>"
    echo "Installed-Size: $(du -sk "$stage" | cut -f 1)"
    echo "Depends: libc6 (>= 2.38), libasound2t64 | libasound2"
    echo "Recommends: ffmpeg"
    echo "Section: sound"
    echo "Priority: optional"
    echo "Homepage: https://github.com/devlato/claudius"
    echo "Description: $summary"
    printf '%s\n' "$description" | sed 's/^/ /'
} > "$deb/DEBIAN/control"
dpkg-deb --build --root-owner-group "$deb" "$out/$name.deb" > /dev/null

# Fedora, openSUSE and the other rpm systems
top=$tmp/rpm
mkdir -p "$top/SPECS"
cat > "$top/SPECS/claudius.spec" <<EOF
# the files are built already: nothing is compiled, stripped or split into debug packages
%global debug_package %{nil}
%global __os_install_post %{nil}
Name: claudius
Version: $package_version
Release: 1
Summary: $summary
License: MIT
URL: https://github.com/devlato/claudius
Requires: alsa-lib
Requires: glibc >= 2.38
Recommends: ffmpeg

%description
$description

%install
cp -a "$stage/." %{buildroot}/

%files
/usr/bin/claudius
/usr/bin/claudius-packs
/usr/share/claudius
/usr/share/applications/claudius-studio.desktop
/usr/share/icons/hicolor/scalable/apps/claudius.svg
/usr/share/icons/hicolor/512x512/apps/claudius.png
%doc /usr/share/doc/claudius
EOF
rpmbuild -bb --quiet --define "_topdir $top" --target "$arch" "$top/SPECS/claudius.spec"
cp "$top"/RPMS/"$arch"/claudius-*.rpm "$out/$name.rpm"

# the AppImage: the same files, and AppRun, which opens the studio when it is started without arguments
fetch() {
    curl -fsSL --proto '=https' --tlsv1.2 --retry 3 -o "$1" "$2"
    echo "$3  $1" | sha256sum --check --strict --quiet
}
fetch "$tmp/appimagetool" \
    "https://github.com/AppImage/appimagetool/releases/download/$appimagetool_version/appimagetool-$arch.AppImage" \
    "$appimagetool_sha256"
fetch "$tmp/runtime" "https://github.com/AppImage/type2-runtime/releases/download/$runtime_version/runtime-$arch" \
    "$runtime_sha256"
chmod +x "$tmp/appimagetool"
appdir=$tmp/claudius.AppDir
cp -R "$stage" "$appdir"
cp "$desktop" "$appdir/"
cp "$root/packaging/icons/claudius-512.png" "$appdir/claudius.png"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
if [ $# -eq 0 ]; then set -- studio; fi
exec "$here/usr/bin/claudius" "$@"
EOF
chmod +x "$appdir/AppRun"
# appimagetool is itself an AppImage: extracted instead of mounted, it needs no FUSE on the build machine
ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 "$tmp/appimagetool" --no-appstream --runtime-file "$tmp/runtime" \
    "$appdir" "$out/$name.AppImage" > /dev/null

for package in "$name.deb" "$name.rpm" "$name.AppImage"; do
    (cd "$out" && sha256sum "$package" > "$package.sha256")
done
echo "linux.sh: $out/$name.deb, .rpm and .AppImage"
