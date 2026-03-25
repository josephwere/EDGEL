function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function formatValue(value) {
  if (value === null || value === undefined) {
    return "null";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value, null, 2);
}

function renderEntries(title, entries) {
  const pairs = Object.entries(entries ?? {});
  return `
    <div class="debug-frame-card">
      <strong>${escapeHtml(title)}</strong>
      <div class="debug-list">
        ${
          pairs.length === 0
            ? `<span class="muted-text">No values available.</span>`
            : pairs
                .map(
                  ([key, value]) => `
                    <div>
                      <span class="search-result-title">${escapeHtml(key)}</span>
                      <span class="file-meta">${escapeHtml(formatValue(value))}</span>
                    </div>
                  `
                )
                .join("")
        }
      </div>
    </div>
  `;
}

export function renderDebugSidebar(container, model) {
  const {
    debug,
    activePath,
    breakpointLines,
    onStartDebug,
    onStep,
    onInspect,
    onSelectFrame
  } = model;

  const frames = debug.snapshot?.frames ?? [];
  const selectedFrame = Math.min(
    Math.max(debug.selectedFrame ?? 0, 0),
    Math.max(frames.length - 1, 0)
  );
  const selectedLocals = frames[selectedFrame]?.locals ?? {};

  container.innerHTML = `
    <div class="sidebar-card">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Debugger</p>
          <h2 class="sidebar-title">Run, inspect, step</h2>
        </div>
        <span class="toolbar-chip">${debug.sessionId ? "Active session" : "Idle"}</span>
      </div>

      <p class="sidebar-subtitle">
        EdgeStudio debugs the active file through GoldEdge Browser APIs without touching the core runtime.
      </p>

      <div class="debug-actions">
        <button type="button" class="tree-action" data-debug-action="start">Start</button>
        <button type="button" class="tree-action" data-debug-action="into" ${debug.sessionId ? "" : "disabled"}>Step Into</button>
        <button type="button" class="tree-action" data-debug-action="over" ${debug.sessionId ? "" : "disabled"}>Step Over</button>
        <button type="button" class="tree-action" data-debug-action="out" ${debug.sessionId ? "" : "disabled"}>Step Out</button>
        <button type="button" class="tree-action" data-debug-action="continue" ${debug.sessionId ? "" : "disabled"}>Continue</button>
      </div>

      <div class="debug-groups">
        <div class="debug-frame-card">
          <strong>Breakpoints</strong>
          <div class="debug-list">
            <span class="file-meta">${escapeHtml(activePath ?? "No file selected")}</span>
            ${
              breakpointLines.length > 0
                ? breakpointLines
                    .map(
                      (line) => `<span class="debug-pill">Line ${escapeHtml(String(line))}</span>`
                    )
                    .join("")
                : `<span class="muted-text">Click line numbers in the editor gutter to add breakpoints.</span>`
            }
          </div>
        </div>

        <div class="debug-frame-card">
          <strong>Call Stack</strong>
          <div class="debug-list">
            ${
              frames.length === 0
                ? `<span class="muted-text">No paused frames yet.</span>`
                : frames
                    .map(
                      (frame, index) => `
                        <button
                          type="button"
                          class="debug-stack-item"
                          data-frame="${index}"
                        >
                          <span class="search-result-title">${escapeHtml(frame.function)}</span>
                          <span class="file-meta">line ${escapeHtml(String(frame.line || 0))} · ${escapeHtml(frame.summary || "paused")}</span>
                        </button>
                      `
                    )
                    .join("")
            }
          </div>
        </div>

        ${renderEntries(`Locals · frame ${selectedFrame}`, selectedLocals)}
        ${renderEntries("Globals", debug.snapshot?.globals ?? {})}

        <div class="debug-frame-card">
          <strong>Inspect expression</strong>
          <div class="debug-actions">
            <input
              id="debugInspectField"
              class="debug-input"
              type="text"
              placeholder="user.name or result"
              value="${escapeHtml(debug.inspectExpr ?? "")}"
            >
            <button type="button" class="tree-action" id="debugInspectAction" ${debug.sessionId ? "" : "disabled"}>Inspect</button>
          </div>
          <div class="debug-list">
            ${
              debug.inspectResult
                ? `
                    <span class="search-result-title">${escapeHtml(debug.inspectExpr || "Result")}</span>
                    <span class="file-meta">${escapeHtml(formatValue(debug.inspectResult))}</span>
                  `
                : `<span class="muted-text">Run an inspect expression to view live values.</span>`
            }
          </div>
        </div>
      </div>
    </div>
  `;

  container.querySelectorAll("[data-debug-action]").forEach((button) => {
    button.addEventListener("click", () => {
      const action = button.dataset.debugAction;
      if (action === "start") {
        onStartDebug();
      } else if (action) {
        onStep(action);
      }
    });
  });

  container.querySelectorAll("[data-frame]").forEach((button) => {
    button.addEventListener("click", () => {
      const frame = Number.parseInt(button.dataset.frame ?? "0", 10);
      onSelectFrame(Number.isFinite(frame) ? frame : 0);
    });
  });

  const inspectField = container.querySelector("#debugInspectField");
  const inspectAction = container.querySelector("#debugInspectAction");
  if (inspectField && inspectAction) {
    const submitInspect = () => onInspect(inspectField.value.trim());
    inspectAction.addEventListener("click", submitInspect);
    inspectField.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        submitInspect();
      }
    });
  }
}
