#!/bin/sh
# usage: packaging/scoop/render.sh VERSION SHA256SUMS > sspur.json
set -eu
[ $# -eq 2 ] || { echo "usage: $0 VERSION SHA256SUMS" >&2; exit 2; }
version=$1
sums=$2
dir=$(cd "$(dirname "$0")" && pwd)

sha() {
  awk -v f="sspur-v$version-$1.zip" '$2 == f || $2 == "*" f { print $1 }' "$sums"
}

x_win=$(sha x86_64-pc-windows-msvc)
a_win=$(sha aarch64-pc-windows-msvc)
[ ${#x_win} -eq 64 ] || { echo "missing x86_64-pc-windows-msvc checksum in $sums" >&2; exit 1; }
if [ ${#a_win} -ne 64 ]; then
  # no arm64 build in this release: drop that architecture from the manifest
  sed -e "s/@VERSION@/$version/g" -e "s/@SHA_X86_64_PC_WINDOWS_MSVC@/$x_win/" "$dir/sspur.json" |
    python3 -c 'import json, sys; m = json.load(sys.stdin); [d.pop("arm64") for d in (m["architecture"], m["autoupdate"]["architecture"])]; print(json.dumps(m, indent=4))'
  exit 0
fi
sed -e "s/@VERSION@/$version/g" \
    -e "s/@SHA_X86_64_PC_WINDOWS_MSVC@/$x_win/" \
    -e "s/@SHA_AARCH64_PC_WINDOWS_MSVC@/$a_win/" \
    "$dir/sspur.json"
