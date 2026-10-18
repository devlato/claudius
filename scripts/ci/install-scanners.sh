#!/bin/sh
# Pinned scanners for CI, each checked against its published SHA-256 before it can run. Linux x86_64 only (the CI
# runners). A version and its checksum change together, in one reviewed commit. With names, installs only those tools
# (`install-scanners.sh gitleaks`); without, all of them. They land on PATH for the job's later steps.
set -eu
BIN="${RUNNER_TEMP:-/tmp}/scanners"
mkdir -p "$BIN"
TOOLS="${*:-gitleaks osv-scanner actionlint}"

wanted() {
    case " $TOOLS " in
        *" $1 "*) return 0 ;;
        *) return 1 ;;
    esac
}

# name, URL, SHA-256 of the download
fetch() {
    file="$BIN/$(basename "$2")"
    curl -fsSL --retry 3 -o "$file" "$2"
    echo "$3  $file" | sha256sum -c --quiet -
    case "$file" in
        *.tar.gz) tar -xzf "$file" -C "$BIN" "$1" ;;
        *) mv "$file" "$BIN/$1" ;;
    esac
    chmod +x "$BIN/$1"
}

if wanted gitleaks; then
    fetch gitleaks https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz \
        551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb
fi
if wanted osv-scanner; then
    fetch osv-scanner https://github.com/google/osv-scanner/releases/download/v2.6.0/osv-scanner_linux_amd64 \
        ca69b3d3cd08f889a49dc0a383122f71cc528b83803671df5fd874d97485b108
fi
if wanted actionlint; then
    fetch actionlint https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz \
        8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8
fi

if [ -n "${GITHUB_PATH:-}" ]; then echo "$BIN" >> "$GITHUB_PATH"; fi
