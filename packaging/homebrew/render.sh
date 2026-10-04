#!/bin/sh
# usage: packaging/homebrew/render.sh VERSION SHA256SUMS > sspur.rb
set -eu
[ $# -eq 2 ] || { echo "usage: $0 VERSION SHA256SUMS" >&2; exit 2; }
version=$1
sums=$2
dir=$(cd "$(dirname "$0")" && pwd)

sha() {
  awk -v f="sspur-v$version-$1.tar.gz" '$2 == f || $2 == "*" f { print $1 }' "$sums"
}

a_mac=$(sha aarch64-apple-darwin)
x_mac=$(sha x86_64-apple-darwin)
a_lnx=$(sha aarch64-unknown-linux-gnu)
x_lnx=$(sha x86_64-unknown-linux-gnu)
for s in "$a_mac" "$x_mac" "$a_lnx" "$x_lnx"; do
  [ ${#s} -eq 64 ] || { echo "missing checksum in $sums" >&2; exit 1; }
done

sed -e "s/@VERSION@/$version/g" \
    -e "s/@SHA_AARCH64_APPLE_DARWIN@/$a_mac/" \
    -e "s/@SHA_X86_64_APPLE_DARWIN@/$x_mac/" \
    -e "s/@SHA_AARCH64_UNKNOWN_LINUX_GNU@/$a_lnx/" \
    -e "s/@SHA_X86_64_UNKNOWN_LINUX_GNU@/$x_lnx/" \
    "$dir/sspur.rb"
