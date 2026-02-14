# Implementation Tasks

## 1. Bootstrap self-hosted SPA skeleton
**Goal**: Create the minimal project structure for a browser-agnostic, static single-page app.
**Implement**: Base HTML page, app entrypoint, static asset layout, and local run instructions aligned with `spec.md` Architecture and Interfaces.
**Verify**: App loads in a browser from local static hosting and displays placeholder UI sections.
**Depends on**: None

## 2. Set up Rust-to-WASM parsing module contract
**Goal**: Define and implement the WASM boundary for DXF bytes input and structured parse output/errors.
**Implement**: Rust crate for parser interface, WASM export functions, and JS interop types matching `spec.md` Data Model (`GeometryEntity`, parse errors).
**Verify**: Passing fixture byte arrays returns structured success/error results in browser runtime.
**Depends on**: 1

## 3. Implement file ingestion UI (drop + picker)
**Goal**: Enable local loading of one or more DXF files with low user effort.
**Implement**: Drag-and-drop zone, file input fallback, `.dxf` filtering, and load event plumbing per `spec.md` Interfaces.
**Verify**: Single and multi-file selection events are captured with metadata in in-memory `LoadedFile` entries.
**Depends on**: 1

## 4. Parse and map DXF data into scene model
**Goal**: Convert loaded files into in-memory scene entities and bounds.
**Implement**: Browser-side orchestration calling WASM parser, mapping output into `GeometryScene` and `GeometryEntity` from `spec.md` Data Model.
**Verify**: Valid fixture files produce non-empty entities and correct aggregate bounds values.
**Depends on**: 2

## 5. Build baseline viewer renderer
**Goal**: Render mapped geometry to the viewer canvas.
**Implement**: Viewer module, render loop, scene composition, and file-to-scene updates as defined in `spec.md` Architecture.
**Verify**: Loading a valid DXF results in visible geometry on canvas.
**Depends on**: 4

## 6. Add pan/zoom/tilt camera controls
**Goal**: Support interactive navigation for inspection workflows.
**Implement**: Input handlers and view-state updates for pan, zoom, and tilt/orbit per `spec.md` Goals and `ViewState`.
**Verify**: User interactions update the viewpoint smoothly without breaking rendering.
**Depends on**: 5

## 7. Implement Focus-to-content behavior
**Goal**: Reframe the camera to all visible geometry on demand.
**Implement**: Bounds-based camera fit calculation and Focus button wiring from `spec.md` Acceptance Criteria.
**Verify**: After arbitrary navigation, pressing Focus fits all visible entities into view.
**Depends on**: 6

## 8. Add dynamic grid and coordinate references
**Goal**: Improve spatial readability across zoom levels.
**Implement**: Faint grid overlay with dynamic spacing and coordinate markers in the viewer module, per `spec.md` Goals.
**Verify**: Grid remains readable at different zoom levels and coordinate references are visible.
**Depends on**: 5

## 9. Harden error handling and warnings UI
**Goal**: Ensure malformed/invalid files fail gracefully without app shutdown.
**Implement**: Warning panel, per-file error statuses, partial multi-file failure handling, and guardrails from `spec.md` Error Handling.
**Verify**: Malformed DXF shows warning, valid files still render, app remains interactive.
**Depends on**: 3

## 10. Add required integration test flow
**Goal**: Satisfy definition of done with browser-driver automation.
**Implement**: Integration test that opens app, loads a DXF fixture, triggers Focus, and asserts graphics are displayed per `spec.md` Testing Plan.
**Verify**: Test passes consistently in CI/local run and fails if rendering pipeline is broken.
**Depends on**: 7

## 11. Add supplemental tests for edge cases
**Goal**: Cover key reliability behaviors beyond the required test.
**Implement**: Integration tests for malformed DXF handling and multi-file partial failure; unit tests for bounds calculation and mapping logic.
**Verify**: Tests pass and specifically assert non-crashing behavior plus warning output.
**Depends on**: 9

## 12. Finalize documentation for local operation
**Goal**: Ensure users can run and use the app with minimal effort.
**Implement**: README usage for local hosting/opening, file loading workflow, supported scope/non-goals, and troubleshooting notes linked to `spec.md` Summary and Non-goals.
**Verify**: A new user can run the app and complete the primary workflow without additional guidance.
**Depends on**: 10
