function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

const PANEL_TABS = [
  { id: "terminal", label: "Terminal" },
  { id: "output", label: "Output" },
  { id: "problems", label: "Problems" },
  { id: "debug", label: "Debug Console" },
  { id: "preview", label: "Preview" },
  { id: "ai", label: "NEUROEDGE" }
];

function formatProblem(problem) {
  const location =
    problem.line || problem.column
      ? `line ${problem.line || 0}, column ${problem.column || 0}`
      : "workspace";
  return `
    <article class="problem-item">
      <span class="problem-title">${escapeHtml(problem.error || "Unknown EDGEL problem")}</span>
      <span class="problem-meta">${escapeHtml(location)}</span>
      ${
        problem.context
          ? `<span class="muted-text">${escapeHtml(`context: ${problem.context}`)}</span>`
          : ""
      }
      ${
        Array.isArray(problem.notes) && problem.notes.length > 0
          ? `<span class="muted-text">${escapeHtml(problem.notes.join(" | "))}</span>`
          : ""
      }
    </article>
  `;
}

export function createBottomPanel({ container, onCommand }) {
  const state = {
    activeTab: "terminal",
    terminalLines: [
      "EdgeStudio terminal is connected to GoldEdge Browser APIs.",
      "Try: edgel run, edgel debug, edgel build --web, edgel ai explain, edgel logs"
    ],
    output: "Run or build the active document to see EDGEL output here.",
    problems: [],
    debug: "Start a debug session to view snapshots, stack frames, and inspection results.",
    previewHtml: "",
    aiNotes: "NEUROEDGE is ready to explain or review the active file."
  };

  function render() {
    container.innerHTML = `
      <div class="bottom-panel-head">
        <div class="panel-tabs">
          ${PANEL_TABS.map(
            (tab) => `
              <button
                type="button"
                class="panel-tab ${tab.id === state.activeTab ? "active" : ""}"
                data-panel-tab="${escapeHtml(tab.id)}"
              >
                ${escapeHtml(tab.label)}
              </button>
            `
          ).join("")}
        </div>
        <span class="muted-text">EdgeStudio panels</span>
      </div>
      <div class="panel-body">${renderBody()}</div>
    `;

    container.querySelectorAll("[data-panel-tab]").forEach((button) => {
      button.addEventListener("click", () => {
        state.activeTab = button.dataset.panelTab ?? state.activeTab;
        render();
      });
    });

    const terminalInput = container.querySelector("#terminalCommand");
    const terminalForm = container.querySelector("#terminalForm");
    if (terminalInput && terminalForm) {
      terminalForm.addEventListener("submit", (event) => {
        event.preventDefault();
        const command = terminalInput.value.trim();
        if (!command) {
          return;
        }
        terminalInput.value = "";
        onCommand(command);
      });
    }

    const preview = container.querySelector("#previewFrame");
    if (preview) {
      preview.srcdoc = state.previewHtml || defaultPreview();
    }
  }

  function renderBody() {
    switch (state.activeTab) {
      case "output":
        return `<pre class="panel-console">${escapeHtml(state.output)}</pre>`;
      case "problems":
        return state.problems.length === 0
          ? `<pre class="panel-console">No current problems.</pre>`
          : `<div class="panel-problems">${state.problems.map(formatProblem).join("")}</div>`;
      case "debug":
        return `<pre class="panel-console">${escapeHtml(state.debug)}</pre>`;
      case "preview":
        return `<iframe id="previewFrame" class="terminal-preview" title="EDGEL preview"></iframe>`;
      case "ai":
        return `<pre class="panel-console">${escapeHtml(state.aiNotes)}</pre>`;
      case "terminal":
      default:
        return `
          <div class="terminal-shell">
            <pre class="terminal-log">${escapeHtml(state.terminalLines.join("\n"))}</pre>
            <form id="terminalForm" class="terminal-form">
              <span class="terminal-prompt">$</span>
              <input
                id="terminalCommand"
                class="terminal-input"
                type="text"
                autocomplete="off"
                placeholder="Run EDGEL commands through GoldEdge Browser"
              >
              <button type="submit" class="terminal-send">Run</button>
            </form>
          </div>
        `;
    }
  }

  function defaultPreview() {
    return "<!doctype html><html><body style=\"font-family:sans-serif;padding:24px\"><h1>EdgeStudio Preview</h1><p>Run an app or build a web bundle to preview EDGEL output here.</p></body></html>";
  }

  render();

  return {
    setActiveTab(tab) {
      state.activeTab = tab;
      render();
    },
    appendTerminal(line) {
      state.terminalLines = [...state.terminalLines, line].slice(-220);
      render();
    },
    setTerminalLines(lines) {
      state.terminalLines = lines.slice(-220);
      render();
    },
    setOutput(text) {
      state.output = text;
      render();
    },
    setProblems(problems) {
      state.problems = problems;
      render();
    },
    setDebugConsole(text) {
      state.debug = text;
      render();
    },
    setPreviewHtml(html) {
      state.previewHtml = html ?? "";
      render();
    },
    setAiNotes(text) {
      state.aiNotes = text;
      render();
    }
  };
}
