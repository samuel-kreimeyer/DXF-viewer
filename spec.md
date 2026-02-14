# Project Specification

## Summary
This project is a self-hosted, single-page web application for locally viewing DXF files in a browser. It is intended for users who need quick, low-effort inspection of DXF geometry without editing or cloud services. Core value is simple, trustworthy local visualization with essential navigation controls.

## Goals
1. Run locally with minimal setup by opening a hosted static page in a browser.
2. Allow users to load one or more `.dxf` files from local disk, including drag-and-drop.
3. Render DXF geometry for visual inspection in a browser-agnostic way.
4. Provide viewport controls for pan, zoom, and tilt/orbit interaction.
5. Provide a "Focus" action that fits loaded geometry into view.
6. Display a faint, dynamically scaled grid with visible coordinate references.
7. Handle malformed DXF input gracefully with clear warnings and no hard crash.

## Non-goals
- Editing DXF geometry or metadata.
- Exporting/saving modified files.
- Supporting non-DXF drawing formats.
- Any backend service, cloud upload, or persistent data storage.

## Users & Use Cases
- Primary user: engineer/technician/designer who needs quick local DXF inspection.
- Use case 1: open app locally, drag a DXF file into the page, inspect geometry immediately.
- Use case 2: load multiple DXF files to compare placement/shape in one view.
- Use case 3: navigate with pan/zoom/tilt and use Focus to recover from disorientation.
- Use case 4: detect invalid input quickly through warning messages when files are malformed.

## Acceptance Criteria
1. Given the app is loaded in a supported browser, when the user drops a valid DXF file, then the geometry is rendered and the file appears in the loaded-file list.
2. Given one or more DXF files are rendered, when the user clicks Focus, then the camera reframes so all currently visible geometry fits in view.
3. Given geometry is visible, when the user pans/zooms/tilts, then the view updates smoothly and remains interactive.
4. Given any zoom level, when the scene is rendered, then a faint grid remains visible and adjusts spacing to stay readable, with coordinate markers shown.
5. Given a malformed DXF file, when the user attempts to load it, then the app shows a warning and continues running without crashing or freezing.

## Architecture
- Deployment model: static, self-hosted SPA served from local/owned infrastructure.
- Runtime structure:
  - Rust core compiled to WebAssembly for DXF parsing and geometry preparation.
  - JavaScript/TypeScript browser shell for file input, event wiring, and rendering loop orchestration.
  - Viewer module for camera controls (pan/zoom/tilt), grid rendering, and scene composition.
  - UI module for drop zone, file picker, warning area, and Focus button.
- Data flow:
  1. User selects/drops local file(s).
  2. Browser shell reads file bytes and sends to WASM parser.
  3. Parser returns geometry primitives or structured parse errors.
  4. Viewer maps primitives to renderable scene objects and updates camera bounds.
  5. UI shows warnings/status and keeps app interactive.
- Constraint notes:
  - No backend, no persistence, no external processing required for normal use.
  - Keep module boundaries simple and maintainable.

## Data Model
No persistent storage is used. In-memory models only:

- `LoadedFile`
  - `id: string` (unique per load event)
  - `name: string` (original filename)
  - `sizeBytes: number` (>0)
  - `status: "loaded" | "error"`
  - `warningMessage?: string`

- `GeometryScene`
  - `entities: GeometryEntity[]`
  - `bounds: { minX: number, minY: number, minZ: number, maxX: number, maxY: number, maxZ: number }`
  - `sourceFileIds: string[]`

- `GeometryEntity`
  - `kind: "line" | "polyline" | "arc" | "circle" | "text" | "other"`
  - `vertices: Array<{ x: number, y: number, z: number }>`
  - `layer?: string`
  - `color?: string`

- `ViewState`
  - `cameraTarget: { x: number, y: number, z: number }`
  - `zoom: number` (>0)
  - `tilt: number`
  - `panOffset: { x: number, y: number }`
  - `gridStep: number` (>0)

## Interfaces
- Browser UI (primary interface):
  - Drag-and-drop zone for one or more `.dxf` files.
  - File picker fallback for browsers/users who do not use drag-and-drop.
  - Canvas viewer area showing geometry + grid + coordinates.
  - Focus button to fit all visible geometry.
  - Warning/status panel for malformed files and load results.
- No CLI, HTTP API, or external config required for v1.

## Error Handling
- Invalid extension or unreadable file: reject load, show warning with filename and reason.
- Malformed DXF parse error: mark file as `error`, show warning, keep existing scene/view responsive.
- Empty/unsupported entity set: show "no renderable entities" warning; do not crash.
- Multi-file partial failure: load valid files, report failed files individually.
- Unexpected runtime/render errors: catch at module boundary, show non-blocking error banner, keep UI available for retry.

## Testing Plan
- Integration test (required by definition of done):
  - Launch browser via driver.
  - Open app page.
  - Load fixture DXF file.
  - Trigger Focus action.
  - Assert rendered graphics are present (e.g., non-empty scene/canvas state).
- Additional integration coverage:
  - Drag-and-drop flow for single and multiple files.
  - Malformed DXF shows warning and does not terminate app.
  - Grid visibility and coordinate marker presence across zoom levels.
- Unit-level checks:
  - Parser output mapping to `GeometryEntity`.
  - Bounds calculation used by Focus behavior.

## Risks & Open Questions
- Assumption: modern browser support includes WebAssembly and required canvas features; exact browser version matrix is not yet specified.
- Risk: large DXF files may impact performance; mitigation is incremental rendering and explicit scope limits.
- Risk: DXF entity variability may exceed initial parser coverage; unsupported entities should degrade gracefully.
- Open decision (deferred): exact camera interaction semantics for "tilt" (2D angle vs. 3D orbit) should be fixed early to avoid UI rework.
