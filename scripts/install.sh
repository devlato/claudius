#!/bin/sh
# Installs claudius from a GitHub release on macOS or Linux.
#
#   curl -fsSL https://github.com/devlato/claudius/releases/latest/download/install.sh | sh
#   curl -fsSL https://github.com/devlato/claudius/releases/latest/download/install.sh | sh -s -- --yes    # also download the packs and models
#
# Options:
#   -y, --yes     run `claudius setup` afterwards without asking (the sample packs and the listening models, ~2 GB)
#   --no-setup    do not offer to run it
#   --headless    on Linux, install the build without the studio window, which needs neither ALSA nor a display
#                 (`claudius serve` still serves the web studio); it is picked anyway when ALSA is not installed
#
# Environment:
#   CLAUDIUS_VERSION       the version to install, e.g. 0.2.0 (default: the latest release)
#   CLAUDIUS_INSTALL_DIR   where the binaries go (default: ~/.local/bin); the web studio goes to
#                          <that>/../share/claudius/web, which is where claudius looks for it
#   CLAUDIUS_DOWNLOAD_URL  where the archives and SHA256SUMS are, an https:// address (default: the GitHub release)

# Everything happens in main, so a download cut short runs nothing.

set -eu

REPO=devlato/claudius
# the oldest glibc the Linux builds run on (ONNX Runtime's prebuilt library needs 2.38)
MIN_GLIBC=2.38

say() {
    printf 'claudius: %s\n' "$*"
}

fail() {
    printf 'claudius: error: %s\n' "$*" >&2
    exit 1
}

has() {
    command -v "$1" >/dev/null 2>&1
}

download() {
    if has curl; then
        curl -fsSL --proto '=https' --tlsv1.2 --retry 3 -o "$2" "$1"
    elif has wget; then
        wget --https-only -q -O "$2" "$1"
    else
        fail "needs curl or wget to download"
    fi
}

sha256() {
    if has sha256sum; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif has shasum; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    elif has openssl; then
        openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1
    else
        fail "needs sha256sum, shasum or openssl to check the download"
    fi
}

target() {
    arch=$(uname -m)
    case $arch in
        x86_64 | amd64) arch=x86_64 ;;
        arm64 | aarch64) arch=aarch64 ;;
        *) fail "there is no build for $arch; build from source: https://github.com/$REPO" ;;
    esac
    case $(uname -s) in
        Darwin)
            # a shell under Rosetta reports x86_64 on Apple silicon
            if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
                arch=aarch64
            fi
            echo "$arch-apple-darwin"
            ;;
        Linux)
            check_glibc
            echo "$arch-unknown-linux-gnu"
            ;;
        MINGW* | MSYS* | CYGWIN*) fail "on Windows, run this in PowerShell: irm https://github.com/$REPO/releases/latest/download/install.ps1 | iex" ;;
        *) fail "there is no build for $(uname -s); build from source: https://github.com/$REPO" ;;
    esac
}

check_glibc() {
    glibc=$(getconf GNU_LIBC_VERSION 2>/dev/null | cut -d ' ' -f 2) || glibc=
    [ -n "$glibc" ] || fail "the Linux builds need glibc (musl systems such as Alpine are not supported)"
    major=${glibc%%.*}
    minor=${glibc#*.}
    minor=${minor%%.*}
    if [ "$major" -lt "${MIN_GLIBC%%.*}" ] || { [ "$major" -eq "${MIN_GLIBC%%.*}" ] && [ "$minor" -lt "${MIN_GLIBC#*.}" ]; }; then
        fail "this system has glibc $glibc and the Linux builds need $MIN_GLIBC or newer (Ubuntu 24.04, Debian 13, Fedora 39 and later)"
    fi
}

# whether ALSA's libasound.so.2, which the build with the studio window links, is installed
has_alsa() {
    for ldconfig in ldconfig /sbin/ldconfig /usr/sbin/ldconfig; do
        if libraries=$("$ldconfig" -p 2>/dev/null); then
            case $libraries in
                *libasound.so.2*) return 0 ;;
                *) return 1 ;;
            esac
        fi
    done
    for lib in /usr/lib/*-linux-gnu/libasound.so.2 /lib/*-linux-gnu/libasound.so.2 /usr/lib64/libasound.so.2 /usr/lib/libasound.so.2; do
        [ -e "$lib" ] && return 0
    done
    return 1
}

# the archive in the SHA256SUMS file $1 whose name ends with $2
archive_in() {
    awk -v suffix="$2" 'substr($2, length($2) - length(suffix) + 1) == suffix { print $2; exit }' "$1"
}

path_advice() {
    case ":$PATH:" in
        *":$1:"*) return ;;
    esac
    say "$1 is not on your PATH. Add it with:"
    case ${SHELL:-} in
        */zsh) echo "  echo 'export PATH=\"$1:\$PATH\"' >> ~/.zshrc && exec zsh" ;;
        */bash)
            # Terminal on macOS starts login shells, which read ~/.bash_profile
            rc=~/.bashrc
            [ "$(uname -s)" = Darwin ] && rc=~/.bash_profile
            echo "  echo 'export PATH=\"$1:\$PATH\"' >> $rc && exec bash"
            ;;
        */fish) echo "  fish_add_path $1" ;;
        *) echo "  export PATH=\"$1:\$PATH\"    (in your shell's startup file)" ;;
    esac
}

main() {
    yes=no
    setup=ask
    headless=no
    for arg in "$@"; do
        case $arg in
            -y | --yes) yes=yes ;;
            --no-setup) setup=no ;;
            --headless) headless=asked ;;
            -h | --help)
                # the comment at the top of this file, when it runs from a file
                sed -n '2,/^$/s/^# \{0,1\}//p' "$0" 2>/dev/null || echo "https://github.com/$REPO/blob/main/scripts/install.sh"
                exit 0
                ;;
            *) fail "unknown option $arg (try --help)" ;;
        esac
    done
    has tar || fail "needs tar"

    target=$(target)
    case $target in
        *-linux-gnu)
            if [ "$headless" = no ] && ! has_alsa; then
                headless=auto
            fi
            ;;
        *)
            if [ "$headless" = asked ]; then
                say "macOS has one build, and it runs \`claudius serve\` without a window as well"
                headless=no
            fi
            ;;
    esac
    # the Intel Mac build has no ears, because ONNX Runtime has no Intel Mac build (xtask/src/dist.rs, NO_EARS)
    ears=yes
    [ "$target" = x86_64-apple-darwin ] && ears=no
    if [ "$ears" = yes ]; then
        downloads="the sample packs and the listening models (about 2 GB)"
    else
        downloads="the sample packs (about 1.1 GB)"
    fi

    version=${CLAUDIUS_VERSION:-}
    version=${version#v}
    if [ -n "${CLAUDIUS_DOWNLOAD_URL:-}" ]; then
        base=${CLAUDIUS_DOWNLOAD_URL%/}
        case $base in
            https://*) ;;
            *) fail "CLAUDIUS_DOWNLOAD_URL has to be an https:// address" ;;
        esac
    elif [ -n "$version" ]; then
        base=https://github.com/$REPO/releases/download/v$version
    else
        base=https://github.com/$REPO/releases/latest/download
    fi
    bin_dir=${CLAUDIUS_INSTALL_DIR:-$HOME/.local/bin}
    bin_dir=${bin_dir%/}
    [ -n "$bin_dir" ] || fail "CLAUDIUS_INSTALL_DIR is /, pick a directory for the binaries"
    case $bin_dir in
        /*) ;;
        *) bin_dir=$PWD/$bin_dir ;;
    esac
    prefix=$(dirname "$bin_dir")
    web_dir=$prefix/share/claudius/web

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    trap 'exit 130' INT TERM

    download "$base/SHA256SUMS" "$tmp/SHA256SUMS" || fail "could not download $base/SHA256SUMS"
    variant=
    [ "$headless" = no ] || variant=-headless
    archive=$(archive_in "$tmp/SHA256SUMS" "-$target$variant.tar.gz")
    if [ -z "$archive" ] && [ "$headless" = auto ]; then
        say "ALSA's libasound.so.2 is not installed, and this release has no headless build to use instead"
        headless=no
        variant=
        archive=$(archive_in "$tmp/SHA256SUMS" "-$target.tar.gz")
    fi
    [ -n "$archive" ] || fail "the release at $base has no ${variant:+headless }build for $target; build from source: https://github.com/$REPO"
    expected=$(awk -v file="$archive" '$2 == file { print $1; exit }' "$tmp/SHA256SUMS")
    version=${archive#claudius-}
    version=${version%"-$target$variant.tar.gz"}

    case $headless in
        asked) say "downloading claudius $version for $target, the headless build (no studio window)" ;;
        auto) say "downloading claudius $version for $target, the headless build (no studio window), because ALSA's libasound.so.2 is not installed" ;;
        *) say "downloading claudius $version for $target" ;;
    esac
    download "$base/$archive" "$tmp/$archive" || fail "could not download $base/$archive"
    actual=$(sha256 "$tmp/$archive")
    [ "$actual" = "$expected" ] || fail "$archive does not match SHA256SUMS (expected $expected, got $actual); nothing was installed"
    say "checksum ok"

    tar -xzf "$tmp/$archive" -C "$tmp"
    unpacked=$tmp/${archive%.tar.gz}
    [ -x "$unpacked/bin/claudius" ] || fail "$archive has no bin/claudius"

    mkdir -p "$bin_dir" "$prefix/share/claudius"
    for file in "$unpacked"/bin/*; do
        # a new file moved into place, so a running claudius keeps its old one
        cp "$file" "$bin_dir/.$(basename "$file").new"
        chmod 755 "$bin_dir/.$(basename "$file").new"
        mv -f "$bin_dir/.$(basename "$file").new" "$bin_dir/$(basename "$file")"
    done
    rm -rf "$web_dir"
    cp -R "$unpacked/share/claudius/web" "$web_dir"

    if ! "$bin_dir/claudius" --version >/dev/null 2>"$tmp/run.log"; then
        cat "$tmp/run.log" >&2
        if grep -q libasound "$tmp/run.log"; then
            fail "claudius is installed in $bin_dir but needs ALSA's libasound.so.2: sudo apt install libasound2t64 (libasound2 on Debian) or sudo dnf install alsa-lib, or run this again with --headless"
        fi
        fail "claudius is installed in $bin_dir but does not start (the error is above)"
    fi

    if [ "$headless" = no ]; then
        say "installed claudius $version"
    else
        say "installed claudius $version, the headless build: no studio window; \`claudius serve\` runs the studio in your browser"
    fi
    echo "  $bin_dir/claudius"
    echo "  $bin_dir/claudius-packs"
    echo "  $web_dir (the web studio)"
    if [ "$ears" = no ]; then
        say "this build has no ears, because ONNX Runtime has no Intel Mac build: verdicts use meters and music theory"
    fi
    path_advice "$bin_dir"
    found=$(command -v claudius 2>/dev/null || true)
    if [ -n "$found" ] && [ "$found" != "$bin_dir/claudius" ]; then
        say "note: $found comes first on your PATH, so \`claudius\` runs that one"
    fi

    # the packs are third-party archives, which tar unpacks with their owners and modes when it runs as root
    if [ "$setup" = ask ] && [ "$(id -u)" -eq 0 ]; then
        say "running as root, so the packs are not downloaded now: run \`claudius setup\` as the user who makes the music"
        setup=no
    fi
    if [ "$setup" = ask ] && [ "$yes" = no ]; then
        if [ -t 0 ]; then
            printf 'claudius: download %s now? [Y/n] ' "$downloads"
            read -r answer || answer=n
            case $answer in
                [nN]*) ;;
                *) yes=yes ;;
            esac
        fi
    fi
    if [ "$setup" = ask ] && [ "$yes" = yes ]; then
        "$bin_dir/claudius" setup --yes </dev/null
    else
        echo
        echo "Next: \`claudius setup\` downloads $downloads,"
        if [ "$headless" = no ]; then
            echo "\`claudius doctor\` checks the installation, and \`claudius studio\` opens the studio."
        else
            echo "\`claudius doctor\` checks the installation, and \`claudius serve\` runs the studio in your browser."
        fi
    fi

    echo
    echo "To uninstall: rm $bin_dir/claudius $bin_dir/claudius-packs && rm -r $web_dir"
    if [ "$ears" = yes ]; then
        echo "What \`claudius setup\` downloaded is in ~/.cache/earshot-rs and the packs folder \`claudius doctor\` shows."
    else
        echo "What \`claudius setup\` downloaded is in the packs folder \`claudius doctor\` shows."
    fi
}

main "$@"
