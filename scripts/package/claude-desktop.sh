#!/bin/sh
# Makes Claudius' extension for Claude Desktop, claudius-<version>.mcpb: the manifest in packaging/claude-desktop,
# with this version, and the icon, in a zip. Opening the file installs it in Claude Desktop, which then starts the
# Claudius app's program as its MCP server.
#
#   scripts/package/claude-desktop.sh <version> <out dir>

set -eu

if [ $# -ne 2 ]; then
    echo "usage: $0 <version> <out dir>" >&2
    exit 2
fi
version=$1
out=$2
root=$(cd "$(dirname "$0")/../.." && pwd)

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
sed "s/{{version}}/$version/g" "$root/packaging/claude-desktop/manifest.json" > "$tmp/manifest.json"
cp "$root/packaging/icons/claudius-512.png" "$tmp/icon.png"
mkdir -p "$out"
mcpb=$(cd "$out" && pwd)/claudius-$version.mcpb
rm -f "$mcpb"
(cd "$tmp" && zip -q -X "$mcpb" manifest.json icon.png)
echo "claude-desktop.sh: $mcpb"
