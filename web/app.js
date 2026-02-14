(function () {
  const fileInput = document.getElementById("fileInput");
  const pickFilesBtn = document.getElementById("pickFilesBtn");
  const dropZone = document.getElementById("dropZone");
  const focusBtn = document.getElementById("focusBtn");
  const statusText = document.getElementById("statusText");
  const fileList = document.getElementById("fileList");
  const warningList = document.getElementById("warningList");
  const coordReadout = document.getElementById("coordReadout");
  const canvas = document.getElementById("viewerCanvas");
  const ctx = canvas.getContext("2d");

  const parser = {
    exports: null,
    ready: false,
  };

  const state = {
    files: [],
    warnings: [],
    entities: [],
    nextFileId: 1,
    camera: {
      targetX: 0,
      targetY: 0,
      zoom: 1,
      tilt: 0,
    },
    pointer: {
      dragging: false,
      mode: "pan",
      prevX: 0,
      prevY: 0,
      worldX: 0,
      worldY: 0,
    },
    render: {
      frameCount: 0,
      drawnSegments: 0,
    },
  };

  const fileColors = [
    "#005f73",
    "#bb3e03",
    "#588157",
    "#7b2cbf",
    "#9c6644",
    "#2a6f97",
  ];

  window.__viewerDebug = {
    get renderCount() {
      return state.render.frameCount;
    },
    get drawnSegments() {
      return state.render.drawnSegments;
    },
    get entityCount() {
      return state.entities.length;
    },
  };

  function updateStatus(text) {
    statusText.textContent = text;
  }

  function addWarning(message) {
    if (!message) {
      return;
    }
    state.warnings.push(message);
    if (state.warnings.length > 50) {
      state.warnings = state.warnings.slice(-50);
    }
    renderWarningList();
  }

  function renderWarningList() {
    warningList.innerHTML = "";
    for (const warning of state.warnings) {
      const li = document.createElement("li");
      li.textContent = warning;
      warningList.appendChild(li);
    }
  }

  function renderFileList() {
    fileList.innerHTML = "";
    for (const file of state.files) {
      const li = document.createElement("li");
      li.textContent = file.name;
      const meta = document.createElement("span");
      meta.textContent = file.status === "loaded" ? `(${file.entityCount} entities)` : "(error)";
      li.appendChild(meta);
      fileList.appendChild(li);
    }
  }

  function resizeCanvas() {
    const ratio = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    canvas.width = Math.max(1, Math.floor(width * ratio));
    canvas.height = Math.max(1, Math.floor(height * ratio));
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  }

  function worldToScreen(point) {
    const c = Math.cos(state.camera.tilt);
    const s = Math.sin(state.camera.tilt);
    const dx = point.x - state.camera.targetX;
    const dy = point.y - state.camera.targetY;
    const halfW = canvas.clientWidth / 2;
    const halfH = canvas.clientHeight / 2;

    return {
      x: halfW + state.camera.zoom * (c * dx + s * dy),
      y: halfH + state.camera.zoom * (s * dx - c * dy),
    };
  }

  function screenDeltaToWorld(dx, dy) {
    const c = Math.cos(state.camera.tilt);
    const s = Math.sin(state.camera.tilt);
    return {
      x: (c * dx + s * dy) / state.camera.zoom,
      y: (s * dx - c * dy) / state.camera.zoom,
    };
  }

  function screenToWorld(screenX, screenY) {
    const rect = canvas.getBoundingClientRect();
    const dx = screenX - rect.left - canvas.clientWidth / 2;
    const dy = screenY - rect.top - canvas.clientHeight / 2;
    const worldDelta = screenDeltaToWorld(dx, dy);
    return {
      x: state.camera.targetX + worldDelta.x,
      y: state.camera.targetY + worldDelta.y,
    };
  }

  function roundCoord(value) {
    return Number(value).toFixed(3);
  }

  function updateCoordReadout(x, y) {
    coordReadout.textContent = `x: ${roundCoord(x)}, y: ${roundCoord(y)}`;
  }

  function boundsOfEntities() {
    if (!state.entities.length) {
      return null;
    }

    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;

    for (const entity of state.entities) {
      for (const point of entity.vertices) {
        minX = Math.min(minX, point[0]);
        minY = Math.min(minY, point[1]);
        maxX = Math.max(maxX, point[0]);
        maxY = Math.max(maxY, point[1]);
      }
    }

    if (!Number.isFinite(minX)) {
      return null;
    }

    return { minX, minY, maxX, maxY };
  }

  function focusAll() {
    const bounds = boundsOfEntities();
    if (!bounds) {
      addWarning("Focus skipped: no renderable entities.");
      return;
    }

    const worldWidth = Math.max(bounds.maxX - bounds.minX, 1e-6);
    const worldHeight = Math.max(bounds.maxY - bounds.minY, 1e-6);
    const usableWidth = Math.max(60, canvas.clientWidth * 0.88);
    const usableHeight = Math.max(60, canvas.clientHeight * 0.88);
    state.camera.targetX = (bounds.minX + bounds.maxX) / 2;
    state.camera.targetY = (bounds.minY + bounds.maxY) / 2;
    state.camera.zoom = Math.max(0.001, Math.min(usableWidth / worldWidth, usableHeight / worldHeight));
  }

  function chooseGridStep() {
    const targetPixels = 90;
    const raw = targetPixels / state.camera.zoom;
    const exponent = Math.floor(Math.log10(raw));
    const base = Math.pow(10, exponent);
    const candidates = [1, 2, 5, 10];

    for (const factor of candidates) {
      if (raw <= base * factor) {
        return base * factor;
      }
    }

    return base * 10;
  }

  function visibleWorldBounds() {
    const corners = [
      screenToWorld(0, 0),
      screenToWorld(canvas.clientWidth, 0),
      screenToWorld(0, canvas.clientHeight),
      screenToWorld(canvas.clientWidth, canvas.clientHeight),
    ];

    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;

    for (const corner of corners) {
      minX = Math.min(minX, corner.x);
      minY = Math.min(minY, corner.y);
      maxX = Math.max(maxX, corner.x);
      maxY = Math.max(maxY, corner.y);
    }

    return { minX, minY, maxX, maxY };
  }

  function drawGrid() {
    const bounds = visibleWorldBounds();
    const step = chooseGridStep();
    const majorStep = step * 5;

    let majorLabelCount = 0;

    ctx.lineWidth = 1;

    for (let x = Math.floor(bounds.minX / step) * step; x <= bounds.maxX; x += step) {
      const start = worldToScreen({ x, y: bounds.minY });
      const end = worldToScreen({ x, y: bounds.maxY });
      const isMajor = Math.abs((x / majorStep) - Math.round(x / majorStep)) < 1e-6;
      ctx.strokeStyle = isMajor ? "rgba(76, 88, 76, 0.22)" : "rgba(76, 88, 76, 0.11)";
      ctx.beginPath();
      ctx.moveTo(start.x, start.y);
      ctx.lineTo(end.x, end.y);
      ctx.stroke();

      if (isMajor && majorLabelCount < 20) {
        const labelPoint = worldToScreen({ x, y: bounds.minY });
        ctx.fillStyle = "rgba(54, 60, 54, 0.68)";
        ctx.font = "11px 'IBM Plex Mono', monospace";
        ctx.fillText(roundCoord(x), labelPoint.x + 2, canvas.clientHeight - 8);
        majorLabelCount += 1;
      }
    }

    majorLabelCount = 0;

    for (let y = Math.floor(bounds.minY / step) * step; y <= bounds.maxY; y += step) {
      const start = worldToScreen({ x: bounds.minX, y });
      const end = worldToScreen({ x: bounds.maxX, y });
      const isMajor = Math.abs((y / majorStep) - Math.round(y / majorStep)) < 1e-6;
      ctx.strokeStyle = isMajor ? "rgba(76, 88, 76, 0.22)" : "rgba(76, 88, 76, 0.11)";
      ctx.beginPath();
      ctx.moveTo(start.x, start.y);
      ctx.lineTo(end.x, end.y);
      ctx.stroke();

      if (isMajor && majorLabelCount < 20) {
        const labelPoint = worldToScreen({ x: bounds.minX, y });
        ctx.fillStyle = "rgba(54, 60, 54, 0.68)";
        ctx.font = "11px 'IBM Plex Mono', monospace";
        ctx.fillText(roundCoord(y), 4, labelPoint.y - 3);
        majorLabelCount += 1;
      }
    }

    const origin = worldToScreen({ x: 0, y: 0 });
    ctx.fillStyle = "rgba(7, 57, 50, 0.8)";
    ctx.fillRect(origin.x - 2, origin.y - 2, 4, 4);
  }

  function drawEntities() {
    state.render.drawnSegments = 0;

    for (const entity of state.entities) {
      const color = fileColors[(entity.fileIndex - 1) % fileColors.length];

      if (entity.kind === "text") {
        drawTextEntity(entity, color);
        continue;
      }

      ctx.strokeStyle = color;
      ctx.lineWidth = 1.4;
      ctx.beginPath();

      let last = null;
      for (let i = 0; i < entity.vertices.length; i += 1) {
        const point = entity.vertices[i];
        const screen = worldToScreen({ x: point[0], y: point[1] });
        if (i === 0) {
          ctx.moveTo(screen.x, screen.y);
        } else {
          ctx.lineTo(screen.x, screen.y);
          if (last) {
            state.render.drawnSegments += 1;
          }
        }
        last = screen;
      }

      ctx.stroke();
    }
  }

  function drawTextEntity(entity, color) {
    if (!entity.text || !Array.isArray(entity.vertices) || entity.vertices.length < 1) {
      return;
    }

    const anchor = worldToScreen({ x: entity.vertices[0][0], y: entity.vertices[0][1] });
    let fontSize = 12;

    if (Array.isArray(entity.vertices[1])) {
      const sizePoint = worldToScreen({ x: entity.vertices[1][0], y: entity.vertices[1][1] });
      const pixelHeight = Math.max(8, Math.abs(sizePoint.y - anchor.y));
      fontSize = Math.max(8, Math.min(28, pixelHeight));
    } else if (typeof entity.textHeight === "number") {
      fontSize = Math.max(8, Math.min(28, entity.textHeight * state.camera.zoom * 0.9));
    }

    ctx.fillStyle = color;
    ctx.font = `${fontSize}px "IBM Plex Mono", monospace`;
    ctx.fillText(entity.text, anchor.x + 2, anchor.y - 2);
    ctx.fillRect(anchor.x - 1, anchor.y - 1, 2, 2);
    state.render.drawnSegments += 1;
  }

  function draw() {
    ctx.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);
    drawGrid();
    drawEntities();
    state.render.frameCount += 1;
  }

  function renderLoop() {
    draw();
    window.requestAnimationFrame(renderLoop);
  }

  function parseWithWasm(bytes) {
    const wasm = parser.exports;
    const ptr = wasm.alloc(bytes.length);
    const mem = new Uint8Array(wasm.memory.buffer, ptr, bytes.length);
    mem.set(bytes);

    wasm.parse_dxf(ptr, bytes.length);
    wasm.dealloc(ptr, bytes.length);

    const resultPtr = wasm.result_ptr();
    const resultLen = wasm.result_len();
    const resultSlice = new Uint8Array(wasm.memory.buffer, resultPtr, resultLen);
    const json = new TextDecoder().decode(resultSlice.slice());

    return JSON.parse(json);
  }

  async function loadWasmParser() {
    try {
      let wasmBytes;
      if (typeof window.DXF_WASM_BASE64 === "string" && window.DXF_WASM_BASE64.length > 0) {
        const raw = atob(window.DXF_WASM_BASE64);
        wasmBytes = new Uint8Array(raw.length);
        for (let i = 0; i < raw.length; i += 1) {
          wasmBytes[i] = raw.charCodeAt(i);
        }
      } else {
        const response = await fetch("./dxf_parser.wasm");
        wasmBytes = new Uint8Array(await response.arrayBuffer());
      }

      const module = await WebAssembly.instantiate(wasmBytes, {});
      parser.exports = module.instance.exports;
      parser.ready = true;
      updateStatus("WASM parser ready");
    } catch (error) {
      updateStatus("Failed to initialize parser");
      addWarning(`WASM initialization failed: ${error.message}`);
      throw error;
    }
  }

  function appendParsedEntities(fileRecord, parsed) {
    if (!Array.isArray(parsed.entities)) {
      return 0;
    }

    let count = 0;
    for (const entity of parsed.entities) {
      if (!Array.isArray(entity.vertices) || entity.vertices.length < 2) {
        continue;
      }
      state.entities.push({
        fileId: fileRecord.id,
        fileIndex: fileRecord.colorIndex,
        kind: entity.kind,
        vertices: entity.vertices,
        text: entity.text,
        textHeight: entity.textHeight,
      });
      count += 1;
    }

    return count;
  }

  async function processFiles(fileCollection) {
    if (!parser.ready) {
      addWarning("Parser not ready yet.");
      return;
    }

    const files = Array.from(fileCollection || []);
    if (!files.length) {
      return;
    }

    for (const file of files) {
      const fileRecord = {
        id: `file-${state.nextFileId}`,
        colorIndex: state.nextFileId,
        name: file.name,
        status: "error",
        entityCount: 0,
      };
      state.nextFileId += 1;

      if (!file.name.toLowerCase().endsWith(".dxf")) {
        addWarning(`${file.name}: skipped because extension is not .dxf`);
        state.files.push(fileRecord);
        continue;
      }

      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        const parsed = parseWithWasm(bytes);

        if (parsed.ok) {
          const entityCount = appendParsedEntities(fileRecord, parsed);
          fileRecord.status = "loaded";
          fileRecord.entityCount = entityCount;
        } else {
          addWarning(`${file.name}: ${parsed.error || "parse error"}`);
          fileRecord.status = "error";
        }

        if (Array.isArray(parsed.warnings)) {
          for (const warning of parsed.warnings) {
            addWarning(`${file.name}: ${warning}`);
          }
        }
      } catch (error) {
        addWarning(`${file.name}: ${error.message}`);
        fileRecord.status = "error";
      }

      state.files.push(fileRecord);
    }

    renderFileList();

    if (state.entities.length) {
      focusAll();
    }

    updateStatus(`${state.entities.length} entities from ${state.files.length} file(s)`);
  }

  function hookCanvasEvents() {
    canvas.addEventListener("contextmenu", (event) => {
      event.preventDefault();
    });

    canvas.addEventListener("mousedown", (event) => {
      state.pointer.dragging = true;
      state.pointer.prevX = event.clientX;
      state.pointer.prevY = event.clientY;
      state.pointer.mode = event.button === 2 || event.shiftKey ? "tilt" : "pan";
    });

    window.addEventListener("mouseup", () => {
      state.pointer.dragging = false;
    });

    canvas.addEventListener("mousemove", (event) => {
      const world = screenToWorld(event.clientX, event.clientY);
      state.pointer.worldX = world.x;
      state.pointer.worldY = world.y;
      updateCoordReadout(world.x, world.y);

      if (!state.pointer.dragging) {
        return;
      }

      const dx = event.clientX - state.pointer.prevX;
      const dy = event.clientY - state.pointer.prevY;
      state.pointer.prevX = event.clientX;
      state.pointer.prevY = event.clientY;

      if (state.pointer.mode === "tilt") {
        state.camera.tilt += dx * 0.008;
      } else {
        const worldDelta = screenDeltaToWorld(dx, dy);
        state.camera.targetX -= worldDelta.x;
        state.camera.targetY -= worldDelta.y;
      }
    });

    canvas.addEventListener(
      "wheel",
      (event) => {
        event.preventDefault();

        const before = screenToWorld(event.clientX, event.clientY);
        const factor = event.deltaY < 0 ? 1.08 : 1 / 1.08;
        state.camera.zoom = Math.max(0.0005, Math.min(1_000_000, state.camera.zoom * factor));
        const after = screenToWorld(event.clientX, event.clientY);

        state.camera.targetX += before.x - after.x;
        state.camera.targetY += before.y - after.y;
      },
      { passive: false }
    );
  }

  function hookUiEvents() {
    pickFilesBtn.addEventListener("click", () => fileInput.click());
    fileInput.addEventListener("change", (event) => processFiles(event.target.files));
    focusBtn.addEventListener("click", focusAll);

    const preventDefaults = (event) => {
      event.preventDefault();
      event.stopPropagation();
    };

    ["dragenter", "dragover", "dragleave", "drop"].forEach((eventName) => {
      dropZone.addEventListener(eventName, preventDefaults);
    });

    ["dragenter", "dragover"].forEach((eventName) => {
      dropZone.addEventListener(eventName, () => dropZone.classList.add("active"));
    });

    ["dragleave", "drop"].forEach((eventName) => {
      dropZone.addEventListener(eventName, () => dropZone.classList.remove("active"));
    });

    dropZone.addEventListener("drop", (event) => {
      const dt = event.dataTransfer;
      if (dt && dt.files) {
        processFiles(dt.files);
      }
    });
  }

  async function init() {
    resizeCanvas();
    window.addEventListener("resize", resizeCanvas);
    hookCanvasEvents();
    hookUiEvents();
    renderFileList();
    renderWarningList();

    try {
      await loadWasmParser();
    } catch {
      // Keep UI active even if parser setup fails.
    }

    renderLoop();
  }

  init();
})();
