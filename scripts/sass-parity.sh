#!/bin/sh
#
# Compares sass-dep's resolution with Dart Sass on real-world libraries.
# Usage: scripts/sass-parity.sh <sass-dep-binary>
# Requires: node, npm, and `npm ci` already run in web/ (provides `sass`).

set -eu

BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
HERE=$(cd "$(dirname "$0")" && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# unpack <spec> <dir>: extracts an npm package into <dir>.
unpack() {
    mkdir -p "$2"
    (cd "$WORK" && npm pack --silent "$1" >"$WORK/pack.out")
    tar xzf "$WORK/$(tail -n 1 "$WORK/pack.out")" -C "$2" --strip-components=1
}

unpack bootstrap@5.3.3 "$WORK/bootstrap"
unpack bulma@1.0.2 "$WORK/bulma"
unpack foundation-sites@6.8.1 "$WORK/foundation"
unpack @angular/material@18.2.0 "$WORK/material/node_modules/@angular/material"
unpack @angular/cdk@18.2.0 "$WORK/material/node_modules/@angular/cdk"

mkdir -p "$WORK/material/src"
cat >"$WORK/material/src/styles.scss" <<'SCSS'
@use "@angular/material" as mat;
@use "@angular/cdk";
@include mat.core();
$theme: mat.define-theme((color: (theme-type: light)));
html { @include mat.all-component-themes($theme); }
SCSS

STATUS=0
check() {
    node "$HERE/sass-parity.mjs" "$BIN" "$@" || STATUS=1
}

for entry in bootstrap bootstrap-grid bootstrap-reboot bootstrap-utilities; do
    check "$WORK/bootstrap" "scss/$entry.scss"
done
check "$WORK/bulma" bulma.scss
check "$WORK/bulma" versions/bulma-prefixed.scss
check "$WORK/foundation" scss/foundation.scss
check "$WORK/material" src/styles.scss --load-path node_modules

exit "$STATUS"
