#!/usr/bin/env bash
set -euo pipefail
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."

cargo build -p web --target wasm32-unknown-unknown --release

rm -rf dist target/web-pkg
wasm-bindgen --target web --no-typescript --out-dir target/web-pkg \
  "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/web.wasm"

wasm-opt -Oz \
  --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
  --enable-mutable-globals --enable-reference-types --enable-multivalue \
  target/web-pkg/web_bg.wasm -o target/web-pkg/web_bg.opt.wasm

hash() { shasum -a 256 "$1" | cut -c1-10; }
mkdir -p dist/assets
glue="web-$(hash target/web-pkg/web.js).js"
wasm="web_bg-$(hash target/web-pkg/web_bg.opt.wasm).wasm"
cp target/web-pkg/web.js "dist/assets/$glue"
cp target/web-pkg/web_bg.opt.wasm "dist/assets/$wasm"

sed -e "s#__GLUE__#/assets/$glue#" -e "s#__WASM__#/assets/$wasm#" web/index.html > dist/index.html
if grep -q '__GLUE__\|__WASM__' dist/index.html; then echo "placeholders left in dist/index.html" >&2; exit 1; fi
cp web/_headers dist/_headers

for f in "dist/assets/$wasm" "dist/assets/$glue"; do
  printf '%s raw=%s gzip=%s\n' "$f" "$(wc -c < "$f" | tr -d ' ')" "$(gzip -9c "$f" | wc -c | tr -d ' ')"
done
