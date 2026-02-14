# DXF Viewer

Self-hosted single-page DXF viewer with Rust + WebAssembly parsing.

## What this does

- Loads one or more local `.dxf` files (drag/drop or file picker)
- Renders geometry in a browser canvas
- Supports pan, zoom, and tilt interaction
- Includes `Focus` to fit all loaded geometry
- Displays a faint dynamic grid with coordinate labels
- Shows warnings for malformed DXF input without crashing

## Project layout

- `web/` static app (`index.html`, `app.js`, `styles.css`, generated wasm assets)
- `wasm-parser/` Rust parser compiled to `wasm32-unknown-unknown`
- `fixtures/` sample DXF files for manual and automated checks
- `tests/` integration test (browser-driver)

## Build WASM

```bash
./scripts/build_wasm.sh
```

This produces:

- `web/dxf_parser.wasm`
- `web/wasm_bytes.js` (embedded base64 bytes)

## Run locally (no backend required)

1. Build WASM (`./scripts/build_wasm.sh`)
2. Open `web/index.html` directly in a browser

The app prefers embedded WASM bytes from `web/wasm_bytes.js`, so it can run from a local file path.

## Optional static hosting

```bash
cd web
python3 -m http.server 4173
```

Open `http://127.0.0.1:4173`.

## Integration test

```bash
npm install
npm run test:integration
```

The test opens the app with a browser driver, loads `fixtures/simple_line.dxf`, zooms, clicks `Focus`, and asserts graphics were drawn.

## Limitations (v1 scope)

- No DXF editing
- No non-DXF format support
- No data persistence/backend
- Parser currently targets common entity types (`LINE`, `LWPOLYLINE`, `POLYLINE` with `VERTEX`/`SEQEND`, `SPLINE`, `HATCH` (boundary paths), `CIRCLE`, `ARC`, `ELLIPSE`, `TEXT`, `MTEXT`, `INSERT` via `BLOCK` expansion, `DIMENSION` fallback geometry, `LEADER`/`MULTILEADER` fallback geometry)
