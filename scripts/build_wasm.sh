#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE_MANIFEST="$ROOT_DIR/wasm-parser/Cargo.toml"
WASM_TARGET_DIR="$ROOT_DIR/wasm-parser/target/wasm32-unknown-unknown/release"
WASM_NAME="dxf_wasm_parser.wasm"
WASM_FILE="$WASM_TARGET_DIR/$WASM_NAME"
WEB_WASM="$ROOT_DIR/web/dxf_parser.wasm"
WEB_BYTES_JS="$ROOT_DIR/web/wasm_bytes.js"

cargo build --manifest-path "$CRATE_MANIFEST" --release --target wasm32-unknown-unknown

cp "$WASM_FILE" "$WEB_WASM"

WASM_BASE64="$(base64 "$WASM_FILE" | tr -d '\n')"
cat > "$WEB_BYTES_JS" <<EOF2
// Generated file. Rebuild with: ./scripts/build_wasm.sh
window.DXF_WASM_BASE64 = "$WASM_BASE64";
EOF2

echo "WASM build complete:"
echo "  Binary: $WEB_WASM"
echo "  Embedded bytes: $WEB_BYTES_JS"
