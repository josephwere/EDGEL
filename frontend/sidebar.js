import { renderDebugSidebar } from "./debug-panel.js";

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function basename(path) {
  return path.split("/").filter(Boolean).pop() ?? path;
}

function dirEntries(paths) {
  const directories = new Set();
  paths.forEach((path) => {
    const parts = path.split("/");
    for (let index = 0; index < parts.length - 1; index += 1) {
      directories.add(parts.slice(0, index + 1).join("/"));
    }
  });
  return Array.from(directories).sort();
}

function flattenTree(paths) {
  const folders = dirEntries(paths).map((path) => ({
    kind: "dir",
    path,
    depth: path.split("/").length - 1,
    label: basename(path)
  }));
  const files = paths.map((path) => ({
    kind: "file",
    path,
    depth: path.split("/").length - 1,
    label: basename(path)
  }));
  return [...folders, ...files].sort((left, right) => left.path.localeCompare(right.path));
}

function renderExplorer(container, state, handlers) {
  const tree = flattenTree(state.projectFiles);
  container.innerHTML = `
    <div class="sidebar-card">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Workspace</p>
          <h2 class="sidebar-title">Explorer</h2>
        </div>
        <span class="toolbar-chip">${escapeHtml(state.projectName || "EDGEL")}</span>
      </div>

      <p class="sidebar-subtitle">
        EdgeStudio runs inside GoldEdge Browser today, but the editor surface is now modular and future-ready for standalone use.
      </p>

      <div class="tree-actions">
        <button type="button" class="tree-action" data-command="new-file">New File</button>
        <button type="button" class="tree-action" data-command="save-file">Save</button>
        <button type="button" class="tree-action" data-command="rename-file">Rename</button>
        <button type="button" class="tree-action" data-command="delete-file">Delete</button>
      </div>

      <div class="sidebar-toolbar">
        <strong>Starter flows</strong>
        <button type="button" class="tree-action" data-command="refresh-project">Refresh</button>
      </div>

      <div class="examples-grid">
        ${state.examples
          .map(
            (example) => `
              <button type="button" class="example-button" data-example="${escapeHtml(example.path)}">
                ${escapeHtml(example.label)}
              </button>
            `
          )
          .join("")}
      </div>

      <div class="sidebar-toolbar">
        <strong>Files</strong>
        <span class="muted-text">${escapeHtml(String(state.projectFiles.length))} tracked files</span>
      </div>

      <div class="tree-view">
        ${tree
          .map(
            (entry) => `
              <div class="tree-row" style="--depth:${entry.depth}">
                <span class="tree-kind">${entry.kind === "dir" ? "DIR" : "EGL"}</span>
                ${
                  entry.kind === "dir"
                    ? `
                      <div>
                        <span class="tree-label">${escapeHtml(entry.label)}</span>
                        <span class="file-meta">${escapeHtml(entry.path)}</span>
                      </div>
                    `
                    : `
                      <button
                        type="button"
                        class="${state.activePath === entry.path ? "active" : ""}"
                        data-open-path="${escapeHtml(entry.path)}"
                      >
                        <span class="tree-label ${state.dirtyPaths.includes(entry.path) ? "dirty" : ""}">${escapeHtml(entry.label)}</span>
                        <span class="file-meta">${escapeHtml(entry.path)}</span>
                      </button>
                    `
                }
              </div>
            `
          )
          .join("")}
      </div>
    </div>
  `;

  container.querySelectorAll("[data-command]").forEach((button) => {
    button.addEventListener("click", () => handlers.onCommand(button.dataset.command ?? ""));
  });
  container.querySelectorAll("[data-open-path]").forEach((button) => {
    button.addEventListener("click", () => handlers.onOpenFile(button.dataset.openPath ?? ""));
  });
  container.querySelectorAll("[data-example]").forEach((button) => {
    button.addEventListener("click", () => handlers.onOpenFile(button.dataset.example ?? ""));
  });
}

function renderSearch(container, state, handlers) {
  container.innerHTML = `
    <div class="sidebar-card">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Workspace</p>
          <h2 class="sidebar-title">Search</h2>
        </div>
        <span class="toolbar-chip">Project-wide</span>
      </div>

      <input
        id="searchField"
        class="search-input"
        type="search"
        autocomplete="off"
        placeholder="Search files, symbols, or text"
        value="${escapeHtml(state.searchQuery)}"
      >

      <div class="search-results">
        ${
          state.searchResults.length === 0
            ? `<p class="muted-text">Type at least two characters to search across the current EDGEL workspace.</p>`
            : state.searchResults
                .map(
                  (result) => `
                    <button type="button" class="search-result" data-search-path="${escapeHtml(result.path)}" data-search-line="${escapeHtml(String(result.line || 1))}">
                      <span class="search-result-title">${escapeHtml(result.path)}</span>
                      <span class="file-meta">line ${escapeHtml(String(result.line || 1))}</span>
                      <span class="muted-text">${escapeHtml(result.preview)}</span>
                    </button>
                  `
                )
                .join("")
        }
      </div>
    </div>
  `;

  const searchField = container.querySelector("#searchField");
  if (searchField) {
    searchField.addEventListener("input", () => handlers.onSearch(searchField.value));
  }

  container.querySelectorAll("[data-search-path]").forEach((button) => {
    button.addEventListener("click", () => {
      const line = Number.parseInt(button.dataset.searchLine ?? "1", 10);
      handlers.onOpenSearchResult(button.dataset.searchPath ?? "", Number.isFinite(line) ? line : 1);
    });
  });
}

function renderSourceControl(container, state, handlers) {
  container.innerHTML = `
    <div class="sidebar-card">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Workspace</p>
          <h2 class="sidebar-title">Source Control</h2>
        </div>
        <span class="toolbar-chip">Git-ready</span>
      </div>

      <p class="sidebar-subtitle">
        EdgeStudio is already tracking unsaved documents. Git wiring stays decoupled and can plug in later without changing the runtime.
      </p>

      <div class="dirty-list">
        ${
          state.dirtyPaths.length === 0
            ? `<div class="dirty-item"><span class="muted-text">No unsaved documents. Your workspace is clean.</span></div>`
            : state.dirtyPaths
                .map(
                  (path) => `
                    <button type="button" class="dirty-item" data-open-path="${escapeHtml(path)}">
                      <span class="dirty-kind">MOD</span>
                      <div>
                        <span class="dirty-path">${escapeHtml(path)}</span>
                        <span class="file-meta">Unsaved editor changes</span>
                      </div>
                    </button>
                  `
                )
                .join("")
        }
      </div>

      <div class="command-grid">
        <button type="button" class="tree-action" data-command="save-file">Save current file</button>
        <button type="button" class="tree-action" data-command="save-all">Save all open files</button>
      </div>
    </div>
  `;

  container.querySelectorAll("[data-open-path]").forEach((button) => {
    button.addEventListener("click", () => handlers.onOpenFile(button.dataset.openPath ?? ""));
  });
  container.querySelectorAll("[data-command]").forEach((button) => {
    button.addEventListener("click", () => handlers.onCommand(button.dataset.command ?? ""));
  });
}

function renderExtensions(container, state, handlers) {
  container.innerHTML = `
    <div class="sidebar-card">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Ecosystem</p>
          <h2 class="sidebar-title">Extensions</h2>
        </div>
        <span class="toolbar-chip">${escapeHtml(String(state.plugins.length))} local plugins</span>
      </div>

      <p class="sidebar-subtitle">
        Extensions connect directly to the EDGEL plugin system. Install creates a local project plugin scaffold, and remove cleans it up.
      </p>

      <div class="plugin-actions">
        <button type="button" class="tree-action" data-command="install-plugin">Install local plugin</button>
        <button type="button" class="tree-action" data-command="refresh-plugins">Refresh plugins</button>
      </div>

      <div class="plugin-list">
        ${
          state.plugins.length === 0
            ? `<div class="plugin-card"><span class="muted-text">No plugins installed yet.</span></div>`
            : state.plugins
                .map(
                  (plugin) => `
                    <div class="plugin-card">
                      <div class="panel-head">
                        <div>
                          <span class="plugin-name">${escapeHtml(plugin.name)}</span>
                          <p class="plugin-path">${escapeHtml(plugin.path)}</p>
                        </div>
                        <button type="button" class="plugin-action" data-remove-plugin="${escapeHtml(plugin.name)}">Remove</button>
                      </div>

                      <div class="plugin-meta">
                        <span class="plugin-pill">${escapeHtml(plugin.version ?? "0.1.0")}</span>
                        <span class="plugin-pill">${escapeHtml(plugin.channel ?? "project")}</span>
                        <span class="plugin-pill">hooks: ${escapeHtml(plugin.hooks.join(", ") || "none")}</span>
                      </div>
                    </div>
                  `
                )
                .join("")
        }
      </div>
    </div>
  `;

  container.querySelectorAll("[data-command]").forEach((button) => {
    button.addEventListener("click", () => handlers.onCommand(button.dataset.command ?? ""));
  });
  container.querySelectorAll("[data-remove-plugin]").forEach((button) => {
    button.addEventListener("click", () => handlers.onRemovePlugin(button.dataset.removePlugin ?? ""));
  });
}

export function renderSidebar(container, state, handlers) {
  switch (state.activeView) {
    case "search":
      renderSearch(container, state, handlers);
      break;
    case "source-control":
      renderSourceControl(container, state, handlers);
      break;
    case "extensions":
      renderExtensions(container, state, handlers);
      break;
    case "debug":
      renderDebugSidebar(container, {
        debug: state.debug,
        activePath: state.activePath,
        breakpointLines: state.breakpointLines,
        onStartDebug: handlers.onStartDebug,
        onStep: handlers.onDebugStep,
        onInspect: handlers.onInspectDebug,
        onSelectFrame: handlers.onSelectDebugFrame
      });
      break;
    case "explorer":
    default:
      renderExplorer(container, state, handlers);
      break;
  }
}
