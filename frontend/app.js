import { createActivityBar } from "./activity-bar.js";
import { createEditorWorkbench } from "./editor.js";
import { renderSidebar } from "./sidebar.js";
import { createBottomPanel } from "./terminal.js";

const ACTIVITY_ITEMS = [
  { id: "explorer", short: "EX", text: "Files", label: "Explorer" },
  { id: "search", short: "SR", text: "Find", label: "Search" },
  { id: "source-control", short: "SC", text: "Track", label: "Source Control" },
  { id: "extensions", short: "PL", text: "Plugins", label: "Extensions" },
  { id: "debug", short: "DB", text: "Trace", label: "Debug" }
];

const EXAMPLES = [
  { label: "Hello World", path: "examples/hello.egl" },
  { label: "Logic", path: "examples/logic.egl" },
  { label: "Mobile App", path: "examples/mobile.egl" },
  { label: "Web API", path: "examples/web.egl" },
  { label: "NEUROEDGE", path: "examples/ai.egl" }
];

const API_BASE = (
  globalThis.EDGESTUDIO_CONFIG?.apiBase ||
  globalThis.location?.origin ||
  "http://127.0.0.1:4040"
).replace(/\/$/, "");

const state = {
  theme: localStorage.getItem("edgestudio-theme") || "dark",
  activity: "explorer",
  projectFiles: [],
  projectName: "EDGEL",
  searchQuery: "",
  searchResults: [],
  plugins: [],
  dirtyPaths: [],
  fileCache: new Map(),
  terminalHistory: [],
  logs: [],
  debug: {
    sessionId: null,
    snapshot: null,
    profile: null,
    inspectExpr: "",
    inspectResult: null,
    selectedFrame: 0
  },
  cursor: { line: 1, column: 1, language: "EDGEL", path: "examples/mobile.egl" },
  palette: { open: false, query: "", index: 0 },
  busy: false
};

const refs = {
  sidebar: document.getElementById("sidebar"),
  editorArea: document.getElementById("editorArea"),
  bottomPanel: document.getElementById("bottomPanel"),
  statusMode: document.getElementById("statusMode"),
  statusPath: document.getElementById("statusPath"),
  statusCursor: document.getElementById("statusCursor"),
  statusLanguage: document.getElementById("statusLanguage"),
  statusDebug: document.getElementById("statusDebug"),
  themeBtn: document.getElementById("themeBtn"),
  splitViewBtn: document.getElementById("splitViewBtn"),
  commandPaletteBtn: document.getElementById("commandPaletteBtn"),
  commandPaletteCloseBtn: document.getElementById("commandPaletteCloseBtn"),
  palette: document.getElementById("commandPalette"),
  paletteInput: document.getElementById("commandPaletteInput"),
  paletteResults: document.getElementById("commandPaletteResults"),
  runBtn: document.getElementById("runBtn"),
  debugBtn: document.getElementById("debugBtn"),
  stopBtn: document.getElementById("stopBtn"),
  buildWebBtn: document.getElementById("buildWebBtn"),
  buildApkBtn: document.getElementById("buildApkBtn"),
  explainBtn: document.getElementById("explainBtn"),
  fixBtn: document.getElementById("fixBtn")
};

const bottomPanel = createBottomPanel({
  container: refs.bottomPanel,
  onCommand: executeTerminalCommand
});

const editor = createEditorWorkbench({
  container: refs.editorArea,
  onCursorChange(cursor) {
    state.cursor = cursor;
    updateStatusBar();
    renderSidebarView();
  },
  onDirtyChange(dirtyPaths) {
    state.dirtyPaths = dirtyPaths;
    renderSidebarView();
  },
  onDocumentChange(document) {
    state.cursor.path = document.path ?? state.cursor.path;
    state.cursor.language = document.language ?? state.cursor.language;
    updateStatusBar();
  },
  onBreakpointChange() {
    renderSidebarView();
  },
  onEmptyAction(action) {
    if (action === "new-file") {
      createNewFile();
    } else {
      openProjectFile("examples/mobile.egl");
    }
  }
});

const activityBar = createActivityBar({
  container: document.getElementById("activityBar"),
  items: ACTIVITY_ITEMS,
  onSelect(activity) {
    state.activity = activity;
    renderSidebarView();
  }
});

let searchTimer = null;

refs.runBtn.addEventListener("click", () => runCurrentFile());
refs.debugBtn.addEventListener("click", () => startDebugSession());
refs.stopBtn.addEventListener("click", stopDebugSession);
refs.buildWebBtn.addEventListener("click", () => buildCurrentFile("web"));
refs.buildApkBtn.addEventListener("click", () => buildCurrentFile("apk"));
refs.explainBtn.addEventListener("click", () => runAiAction("explain"));
refs.fixBtn.addEventListener("click", () => runAiAction("fix"));
refs.splitViewBtn.addEventListener("click", () => {
  const split = editor.toggleSplit();
  refs.splitViewBtn.textContent = split ? "Single View" : "Split View";
});
refs.themeBtn.addEventListener("click", toggleTheme);
refs.commandPaletteBtn.addEventListener("click", () => {
  if (state.palette.open) {
    closeCommandPalette();
  } else {
    openCommandPalette();
  }
});
refs.commandPaletteCloseBtn.addEventListener("click", closeCommandPalette);

refs.palette.addEventListener("click", (event) => {
  if (event.target === refs.palette) {
    closeCommandPalette();
  }
});

refs.paletteInput.addEventListener("input", () => {
  state.palette.query = refs.paletteInput.value;
  state.palette.index = 0;
  renderCommandPalette();
});

refs.paletteInput.addEventListener("keydown", (event) => {
  const commands = visibleCommands();
  if (event.key === "ArrowDown") {
    event.preventDefault();
    state.palette.index = Math.min(state.palette.index + 1, Math.max(commands.length - 1, 0));
    renderCommandPalette();
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    state.palette.index = Math.max(state.palette.index - 1, 0);
    renderCommandPalette();
  } else if (event.key === "Enter") {
    event.preventDefault();
    const command = commands[state.palette.index];
    if (command) {
      closeCommandPalette();
      command.run();
    }
  } else if (event.key === "Escape") {
    event.preventDefault();
    closeCommandPalette();
  }
});

document.addEventListener("keydown", (event) => {
  const modifier = event.ctrlKey || event.metaKey;
  if (modifier && event.shiftKey && event.key.toLowerCase() === "p") {
    event.preventDefault();
    openCommandPalette();
    return;
  }
  if (modifier && event.key.toLowerCase() === "s") {
    event.preventDefault();
    saveActiveDocument();
    return;
  }
  if (event.key === "F5") {
    event.preventDefault();
    runCurrentFile();
    return;
  }
  if (event.key === "F10") {
    event.preventDefault();
    if (state.debug.sessionId) {
      stepDebug("over");
    }
    return;
  }
  if (event.key === "F11" && event.shiftKey) {
    event.preventDefault();
    if (state.debug.sessionId) {
      stepDebug("out");
    }
    return;
  }
  if (event.key === "F11") {
    event.preventDefault();
    if (state.debug.sessionId) {
      stepDebug("into");
    } else {
      startDebugSession();
    }
  }
});

bootstrap().catch((error) => {
  setStatus(`Launch error: ${String(error)}`);
  appendTerminal(`[fatal] ${String(error)}`);
});

async function bootstrap() {
  applyTheme(state.theme);
  updateStatusBar();
  await Promise.all([loadProjectFiles(), loadPlugins(), refreshLogs()]);
  await openInitialFile();
  renderSidebarView();
  renderCommandPalette();
  window.setInterval(() => {
    refreshLogs().catch(() => {});
  }, 12000);
}

function setBusy(isBusy, label = null) {
  state.busy = isBusy;
  document.querySelectorAll(".toolbar-button").forEach((button) => {
    button.disabled = isBusy;
  });
  if (label) {
    setStatus(label);
  }
}

function setStatus(label) {
  refs.statusMode.textContent = label;
}

function updateStatusBar() {
  const active = editor.getActiveDocument();
  refs.statusPath.textContent = active?.path ?? state.cursor.path ?? "No file open";
  refs.statusCursor.textContent = `Ln ${state.cursor.line || 1}, Col ${state.cursor.column || 1}`;
  refs.statusLanguage.textContent = active?.language ?? state.cursor.language ?? "EDGEL";
  refs.statusDebug.textContent = state.debug.sessionId
    ? `Debug line ${state.debug.snapshot?.line || 0}`
    : "Debug idle";
}

function applyTheme(theme) {
  state.theme = theme;
  document.body.dataset.theme = theme;
  localStorage.setItem("edgestudio-theme", theme);
  refs.themeBtn.textContent = theme === "dark" ? "Light Theme" : "Dark Theme";
}

function toggleTheme() {
  applyTheme(state.theme === "dark" ? "light" : "dark");
}

async function requestJson(url, options = {}) {
  const response = await fetch(apiUrl(url), options);
  return response.json();
}

function apiUrl(path) {
  if (/^https?:\/\//.test(path)) {
    return path;
  }
  return `${API_BASE}${path.startsWith("/") ? path : `/${path}`}`;
}

function appendTerminal(line) {
  state.terminalHistory = [...state.terminalHistory, line].slice(-160);
  syncTerminalPanel();
}

function syncTerminalPanel() {
  const lines = [
    "EdgeStudio terminal is connected to GoldEdge Browser APIs.",
    ...state.terminalHistory,
    ...state.logs.slice(-12).map((line) => `[log] ${line}`)
  ];
  bottomPanel.setTerminalLines(lines);
}

async function loadProjectFiles() {
  const data = await requestJson("/api/project?action=list");
  if (!data.ok) {
    throw new Error(data.error || "Failed to load project files");
  }
  state.projectFiles = data.files ?? [];
  state.projectName = basename(data.root ?? "EDGEL");
}

async function loadPlugins() {
  const data = await requestJson("/api/plugins");
  if (!data.ok) {
    appendTerminal(`[plugins] ${data.error || "Failed to load plugins"}`);
    return;
  }
  state.plugins = data.plugins ?? [];
  renderSidebarView();
}

async function refreshLogs() {
  const data = await requestJson("/api/logs");
  if (!data.ok) {
    return;
  }
  state.logs = data.logs ?? [];
  syncTerminalPanel();
}

async function openInitialFile() {
  const preferred = state.projectFiles.includes("examples/mobile.egl")
    ? "examples/mobile.egl"
    : state.projectFiles.find((path) => path.endsWith(".egl")) ?? state.projectFiles[0];
  if (preferred) {
    await openProjectFile(preferred);
  }
}

async function readProjectFile(path, force = false) {
  if (!force && state.fileCache.has(path)) {
    return state.fileCache.get(path);
  }
  const data = await requestJson(`/api/project?action=read&path=${encodeURIComponent(path)}`);
  if (!data.ok) {
    throw new Error(data.error || `Failed to read ${path}`);
  }
  state.fileCache.set(path, data.content ?? "");
  return data.content ?? "";
}

async function openProjectFile(path, pane) {
  if (!path) {
    return;
  }
  try {
    const content = await readProjectFile(path);
    editor.openDocument({
      id: path,
      path,
      label: basename(path),
      content
    }, { pane, saved: true });
    setStatus(`Opened ${path}`);
    state.cursor.path = path;
    renderSidebarView();
    updateStatusBar();
  } catch (error) {
    setStatus("Open failed");
    bottomPanel.setProblems([{ error: String(error) }]);
    bottomPanel.setActiveTab("problems");
  }
}

async function saveDocument(document) {
  if (!document?.path) {
    throw new Error("This tab is not backed by a project file.");
  }
  const url = `/api/project?action=write&path=${encodeURIComponent(document.path)}`;
  const data = await requestJson(url, {
    method: "POST",
    headers: { "Content-Type": "text/plain; charset=utf-8" },
    body: document.content
  });
  if (!data.ok) {
    throw new Error(data.error || `Failed to save ${document.path}`);
  }
  state.fileCache.set(document.path, document.content);
  editor.markSaved(document.id ?? document.path, document.content);
  setStatus(`Saved ${document.path}`);
  appendTerminal(`[save] ${document.path}`);
}

async function saveActiveDocument() {
  const document = editor.getActiveDocument();
  if (!document) {
    return;
  }
  try {
    await saveDocument(document);
  } catch (error) {
    setStatus("Save failed");
    bottomPanel.setProblems([{ error: String(error) }]);
    bottomPanel.setActiveTab("problems");
  }
}

async function saveAllDocuments() {
  const documents = editor.listDocuments().filter((document) => document.dirty && document.path);
  for (const document of documents) {
    await saveDocument(document);
  }
}

function activeDocument() {
  return editor.getActiveDocument();
}

async function ensureSavedForExecution(document) {
  if (document?.dirty && document.path) {
    await saveDocument(document);
  }
}

function buildSourceUrl(base, document, extra = {}) {
  const query = new URLSearchParams();
  if (document?.path) {
    query.set("path", document.path);
  }
  Object.entries(extra).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== "") {
      query.set(key, String(value));
    }
  });
  const queryString = query.toString();
  if (!queryString) {
    return base;
  }
  return `${base}${base.includes("?") ? "&" : "?"}${queryString}`;
}

function flattenDiagnostic(diagnostic, sink = []) {
  if (!diagnostic) {
    return sink;
  }
  sink.push({
    error: diagnostic.error,
    line: diagnostic.line,
    column: diagnostic.column,
    context: diagnostic.context,
    notes: diagnostic.notes ?? []
  });
  (diagnostic.related ?? []).forEach((related) => flattenDiagnostic(related, sink));
  return sink;
}

function diagnosticText(diagnostic) {
  return flattenDiagnostic(diagnostic)
    .map((problem) => {
      const parts = [problem.error || "EDGEL error"];
      if (problem.line || problem.column) {
        parts.push(`line ${problem.line || 0}, column ${problem.column || 0}`);
      }
      if (problem.context) {
        parts.push(`context: ${problem.context}`);
      }
      if (Array.isArray(problem.notes) && problem.notes.length > 0) {
        parts.push(`notes: ${problem.notes.join(" | ")}`);
      }
      return parts.join("\n");
    })
    .join("\n\n");
}

function runtimeText(data) {
  const sections = [];
  if (data.summary) {
    sections.push(data.summary);
  }
  if (Array.isArray(data.console) && data.console.length > 0) {
    sections.push(data.console.join("\n"));
  }
  if (Array.isArray(data.files) && data.files.length > 0) {
    sections.push(`Files:\n${data.files.join("\n")}`);
  }
  if (Array.isArray(data.trace) && data.trace.length > 0) {
    sections.push(`Trace:\n${data.trace.join("\n")}`);
  }
  if (Array.isArray(data.pluginLogs) && data.pluginLogs.length > 0) {
    sections.push(`Plugin Logs:\n${data.pluginLogs.join("\n")}`);
  }
  return sections.join("\n\n");
}

function debugConsoleText() {
  if (!state.debug.snapshot) {
    return "No active debug session.";
  }
  const snapshot = state.debug.snapshot;
  const lines = [];
  lines.push(snapshot.line > 0 ? `[DEBUG] Line ${snapshot.line} -> ${snapshot.summary}` : `[DEBUG] ${snapshot.summary}`);
  lines.push(`[DEBUG] Instruction: ${snapshot.instruction}`);
  if (snapshot.pauseReason) {
    lines.push(`[DEBUG] Pause: ${snapshot.pauseReason}`);
  }
  lines.push("");
  lines.push("Frames:");
  if ((snapshot.frames ?? []).length === 0) {
    lines.push("  <no stack frames>");
  } else {
    snapshot.frames.forEach((frame, index) => {
      const marker = index === state.debug.selectedFrame ? ">" : " ";
      lines.push(`${marker} ${index}: ${frame.function} line ${frame.line || 0} -> ${frame.summary}`);
    });
  }
  lines.push("");
  lines.push("Globals:");
  const globals = Object.entries(snapshot.globals ?? {});
  if (globals.length === 0) {
    lines.push("  <no globals>");
  } else {
    globals.forEach(([key, value]) => lines.push(`  ${key} = ${formatValue(value)}`));
  }
  if (state.debug.inspectExpr) {
    lines.push("");
    lines.push(`Inspect ${state.debug.inspectExpr}: ${formatValue(state.debug.inspectResult)}`);
  }
  if (state.debug.profile) {
    const profile = state.debug.profile;
    lines.push("");
    lines.push(
      `Profile: instructions=${profile.instruction_count}, functions=${profile.function_calls}, builtins=${profile.builtin_calls}, max_stack=${profile.max_stack_depth}, elapsed_ms=${profile.elapsed_ms}`
    );
  }
  return lines.join("\n");
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

function syncDebugUi() {
  bottomPanel.setDebugConsole(debugConsoleText());
  renderSidebarView();
  updateStatusBar();
}

async function runCurrentFile() {
  const document = activeDocument();
  if (!document) {
    return;
  }
  setBusy(true, "Running");
  try {
    await ensureSavedForExecution(document);
    const data = await requestJson(buildSourceUrl("/api/run", document), {
      method: "POST",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: document.content
    });
    if (!data.ok) {
      throw data;
    }
    bottomPanel.setOutput(runtimeText(data));
    bottomPanel.setProblems([]);
    bottomPanel.setPreviewHtml(data.html ?? "");
    bottomPanel.setActiveTab(data.html ? "preview" : "output");
    appendTerminal(`[run] ${document.path || document.label}`);
    if (data.summary) {
      setStatus(data.summary);
    }
  } catch (error) {
    handleRuntimeFailure(error, "Run failed");
  } finally {
    setBusy(false);
  }
}

async function buildCurrentFile(target) {
  const document = activeDocument();
  if (!document) {
    return;
  }
  setBusy(true, target === "web" ? "Building web" : "Building APK");
  try {
    await ensureSavedForExecution(document);
    const data = await requestJson(buildSourceUrl(`/api/build?target=${encodeURIComponent(target)}`, document), {
      method: "POST",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: document.content
    });
    if (!data.ok) {
      throw data;
    }
    bottomPanel.setOutput(runtimeText(data));
    bottomPanel.setProblems([]);
    bottomPanel.setActiveTab("output");
    appendTerminal(`[build] ${target} from ${document.path || document.label}`);
    setStatus(data.summary || "Build complete");
  } catch (error) {
    handleRuntimeFailure(error, "Build failed");
  } finally {
    setBusy(false);
  }
}

async function runAiAction(action) {
  const document = activeDocument();
  if (!document) {
    return;
  }
  setBusy(true, action === "explain" ? "Explaining with NEUROEDGE" : "Reviewing with NEUROEDGE");
  try {
    const data = await requestJson(action === "explain" ? "/api/ai/explain" : "/api/ai/fix", {
      method: "POST",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: document.content
    });
    if (!data.ok) {
      throw data;
    }
    bottomPanel.setAiNotes(data.text || "");
    bottomPanel.setActiveTab("ai");
    appendTerminal(`[ai] ${action} on ${document.path || document.label}`);
    setStatus("NEUROEDGE ready");
  } catch (error) {
    handleRuntimeFailure(error, "AI action failed");
  } finally {
    setBusy(false);
  }
}

async function startDebugSession() {
  const document = activeDocument();
  if (!document) {
    return;
  }
  setBusy(true, "Starting debugger");
  try {
    await ensureSavedForExecution(document);
    const breakpoints = editor.getBreakpoints(document.path);
    const data = await requestJson(
      buildSourceUrl("/api/debug/start", document, {
        breakpoints: breakpoints.join(",")
      }),
      {
        method: "POST",
        headers: { "Content-Type": "text/plain; charset=utf-8" },
        body: document.content
      }
    );
    if (!data.ok) {
      throw data;
    }
    state.debug = {
      sessionId: data.session,
      snapshot: data.snapshot ?? null,
      profile: data.profile ?? null,
      inspectExpr: "",
      inspectResult: null,
      selectedFrame: 0
    };
    bottomPanel.setPreviewHtml(data.html ?? "");
    bottomPanel.setOutput(runtimeText(data));
    bottomPanel.setActiveTab("debug");
    appendTerminal(`[debug] started ${document.path || document.label}`);
    syncDebugUi();
    setStatus("Debugger ready");
  } catch (error) {
    handleRuntimeFailure(error, "Debug failed");
  } finally {
    setBusy(false);
  }
}

function stopDebugSession() {
  state.debug = {
    sessionId: null,
    snapshot: null,
    profile: null,
    inspectExpr: "",
    inspectResult: null,
    selectedFrame: 0
  };
  syncDebugUi();
  appendTerminal("[debug] session cleared");
  setStatus("Debugger stopped");
}

async function stepDebug(action) {
  if (!state.debug.sessionId) {
    return;
  }
  setBusy(true, `Debug ${action}`);
  try {
    const data = await requestJson(
      `/api/debug/step?session=${encodeURIComponent(state.debug.sessionId)}&action=${encodeURIComponent(action)}`,
      { method: "POST" }
    );
    if (!data.ok) {
      throw data;
    }
    state.debug.snapshot = data.snapshot ?? null;
    state.debug.selectedFrame = data.selectedFrame ?? state.debug.selectedFrame;
    bottomPanel.setActiveTab("debug");
    syncDebugUi();
    setStatus(data.done ? "Debug complete" : "Debugger ready");
  } catch (error) {
    handleRuntimeFailure(error, "Debug step failed");
  } finally {
    setBusy(false);
  }
}

async function inspectDebug(expr) {
  if (!state.debug.sessionId) {
    return;
  }
  if (!expr) {
    setStatus("Enter an expression to inspect");
    return;
  }
  setBusy(true, "Inspecting");
  try {
    const url =
      `/api/debug/inspect?session=${encodeURIComponent(state.debug.sessionId)}` +
      `&expr=${encodeURIComponent(expr)}` +
      `&frame=${encodeURIComponent(String(state.debug.selectedFrame || 0))}`;
    const data = await requestJson(url);
    if (!data.ok) {
      throw data;
    }
    state.debug.inspectExpr = expr;
    state.debug.inspectResult = data.value;
    state.debug.snapshot = data.snapshot ?? state.debug.snapshot;
    syncDebugUi();
    bottomPanel.setActiveTab("debug");
    setStatus("Inspect ready");
  } catch (error) {
    handleRuntimeFailure(error, "Inspect failed");
  } finally {
    setBusy(false);
  }
}

function selectDebugFrame(frame) {
  state.debug.selectedFrame = frame;
  syncDebugUi();
}

function handleRuntimeFailure(error, fallbackLabel) {
  const diagnostic = error?.ok === false ? error : { error: String(error) };
  bottomPanel.setProblems(flattenDiagnostic(diagnostic));
  bottomPanel.setOutput(diagnosticText(diagnostic));
  bottomPanel.setActiveTab("problems");
  appendTerminal(`[error] ${diagnostic.error || String(error)}`);
  setStatus(fallbackLabel);
}

async function createNewFile() {
  const suggestedDir = activeDocument()?.path?.split("/").slice(0, -1).join("/") || "src";
  const path = window.prompt("Create file at path:", `${suggestedDir}/new-file.egl`);
  if (!path) {
    return;
  }
  try {
    const data = await requestJson(`/api/project?action=write&path=${encodeURIComponent(path)}`, {
      method: "POST",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: ""
    });
    if (!data.ok) {
      throw new Error(data.error || "Failed to create file");
    }
    state.fileCache.set(path, "");
    await loadProjectFiles();
    await openProjectFile(path);
  } catch (error) {
    handleRuntimeFailure({ error: String(error) }, "Create failed");
  }
}

async function renameActiveFile() {
  const document = activeDocument();
  if (!document?.path) {
    return;
  }
  const nextPath = window.prompt("Rename file to:", document.path);
  if (!nextPath || nextPath === document.path) {
    return;
  }
  try {
    const data = await requestJson(
      `/api/project?action=rename&path=${encodeURIComponent(document.path)}&to=${encodeURIComponent(nextPath)}`,
      { method: "POST" }
    );
    if (!data.ok) {
      throw new Error(data.error || "Rename failed");
    }
    const cached = state.fileCache.get(document.path);
    state.fileCache.delete(document.path);
    if (cached !== undefined) {
      state.fileCache.set(nextPath, cached);
    }
    editor.renameDocumentPath(document.path, nextPath);
    await loadProjectFiles();
    renderSidebarView();
    updateStatusBar();
    setStatus(`Renamed to ${nextPath}`);
  } catch (error) {
    handleRuntimeFailure({ error: String(error) }, "Rename failed");
  }
}

async function deleteActiveFile() {
  const document = activeDocument();
  if (!document?.path) {
    return;
  }
  const confirmed = window.confirm(`Delete ${document.path}?`);
  if (!confirmed) {
    return;
  }
  try {
    const data = await requestJson(
      `/api/project?action=delete&path=${encodeURIComponent(document.path)}`,
      { method: "POST" }
    );
    if (!data.ok) {
      throw new Error(data.error || "Delete failed");
    }
    state.fileCache.delete(document.path);
    editor.removeDocument(document.path);
    await loadProjectFiles();
    renderSidebarView();
    setStatus(`Deleted ${document.path}`);
  } catch (error) {
    handleRuntimeFailure({ error: String(error) }, "Delete failed");
  }
}

async function installPlugin() {
  const name = window.prompt("Plugin name:", "logger");
  if (!name) {
    return;
  }
  setBusy(true, `Installing plugin ${name}`);
  try {
    const data = await requestJson(`/api/plugins?action=install&name=${encodeURIComponent(name)}`, {
      method: "POST"
    });
    if (!data.ok) {
      throw data;
    }
    appendTerminal(`[plugin] installed ${name}`);
    await Promise.all([loadPlugins(), loadProjectFiles()]);
    setStatus(data.summary || "Plugin installed");
  } catch (error) {
    handleRuntimeFailure(error, "Plugin install failed");
  } finally {
    setBusy(false);
  }
}

async function removePlugin(name) {
  if (!name) {
    return;
  }
  const confirmed = window.confirm(`Remove plugin ${name}?`);
  if (!confirmed) {
    return;
  }
  setBusy(true, `Removing plugin ${name}`);
  try {
    const data = await requestJson(`/api/plugins?action=remove&name=${encodeURIComponent(name)}`, {
      method: "POST"
    });
    if (!data.ok) {
      throw data;
    }
    appendTerminal(`[plugin] removed ${name}`);
    await Promise.all([loadPlugins(), loadProjectFiles()]);
    setStatus(data.summary || "Plugin removed");
  } catch (error) {
    handleRuntimeFailure(error, "Plugin remove failed");
  } finally {
    setBusy(false);
  }
}

function renderSidebarView() {
  const document = activeDocument();
  renderSidebar(refs.sidebar, {
    activeView: state.activity,
    projectFiles: state.projectFiles,
    projectName: state.projectName,
    activePath: document?.path ?? state.cursor.path,
    dirtyPaths: state.dirtyPaths,
    searchQuery: state.searchQuery,
    searchResults: state.searchResults,
    plugins: state.plugins,
    examples: EXAMPLES,
    debug: state.debug,
    breakpointLines: document?.path ? editor.getBreakpoints(document.path) : []
  }, {
    onOpenFile(path) {
      openProjectFile(path);
    },
    onSearch(query) {
      queueSearch(query);
    },
    onOpenSearchResult(path, line) {
      openProjectFile(path).then(() => editor.focusLine(line)).catch(() => {});
    },
    onCommand(command) {
      runSidebarCommand(command);
    },
    onRemovePlugin(name) {
      removePlugin(name);
    },
    onStartDebug() {
      startDebugSession();
    },
    onDebugStep(action) {
      stepDebug(action);
    },
    onInspectDebug(expr) {
      inspectDebug(expr);
    },
    onSelectDebugFrame(frame) {
      selectDebugFrame(frame);
    }
  });
}

function runSidebarCommand(command) {
  switch (command) {
    case "new-file":
      createNewFile();
      break;
    case "save-file":
      saveActiveDocument();
      break;
    case "save-all":
      saveAllDocuments().catch((error) => handleRuntimeFailure({ error: String(error) }, "Save all failed"));
      break;
    case "rename-file":
      renameActiveFile();
      break;
    case "delete-file":
      deleteActiveFile();
      break;
    case "refresh-project":
      loadProjectFiles().then(renderSidebarView).catch((error) => handleRuntimeFailure({ error: String(error) }, "Refresh failed"));
      break;
    case "install-plugin":
      installPlugin();
      break;
    case "refresh-plugins":
      loadPlugins();
      break;
    default:
      break;
  }
}

function queueSearch(query) {
  state.searchQuery = query;
  renderSidebarView();
  if (searchTimer) {
    window.clearTimeout(searchTimer);
  }
  searchTimer = window.setTimeout(() => {
    runSearch(query).catch((error) => {
      state.searchResults = [{ path: "Search", line: 1, preview: String(error) }];
      renderSidebarView();
    });
  }, 160);
}

async function runSearch(query) {
  const term = query.trim().toLowerCase();
  if (term.length < 2) {
    state.searchResults = [];
    renderSidebarView();
    return;
  }

  const candidateFiles = state.projectFiles.filter(isSearchableFile).slice(0, 180);
  const matches = [];

  for (const path of candidateFiles) {
    if (path.toLowerCase().includes(term)) {
      matches.push({ path, line: 1, preview: "File name match" });
    }
    if (matches.length >= 40) {
      break;
    }
    const content = await readProjectFile(path);
    const lines = content.split("\n");
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].toLowerCase().includes(term)) {
        matches.push({ path, line: index + 1, preview: lines[index].trim() || "<blank line>" });
        if (matches.length >= 40) {
          break;
        }
      }
    }
    if (matches.length >= 40) {
      break;
    }
  }

  state.searchResults = matches;
  renderSidebarView();
}

function isSearchableFile(path) {
  return [".egl", ".md", ".json", ".js", ".css", ".html", ".rs"].some((ext) => path.endsWith(ext));
}

function basename(path) {
  return path.split("/").filter(Boolean).pop() ?? path;
}

async function executeTerminalCommand(raw) {
  appendTerminal(`$ ${raw}`);
  const command = raw.trim();
  if (!command) {
    return;
  }
  if (command === "help") {
    appendTerminal("Supported commands: edgel run, edgel debug, edgel build --web, edgel build --apk, edgel ai explain, edgel ai fix, edgel logs, edgel plugin list");
    return;
  }
  if (command.startsWith("edgel run")) {
    await runCurrentFile();
    return;
  }
  if (command.startsWith("edgel debug")) {
    await startDebugSession();
    return;
  }
  if (command.startsWith("edgel build --web")) {
    await buildCurrentFile("web");
    return;
  }
  if (command.startsWith("edgel build --apk")) {
    await buildCurrentFile("apk");
    return;
  }
  if (command.startsWith("edgel ai explain")) {
    await runAiAction("explain");
    return;
  }
  if (command.startsWith("edgel ai fix")) {
    await runAiAction("fix");
    return;
  }
  if (command === "edgel logs") {
    await refreshLogs();
    return;
  }
  if (command === "edgel plugin list") {
    await loadPlugins();
    appendTerminal(`Installed plugins: ${state.plugins.map((plugin) => plugin.name).join(", ") || "none"}`);
    return;
  }
  appendTerminal(`Unsupported command: ${command}`);
}

function commandDefinitions() {
  return [
    { id: "run", label: "Run EDGEL file", description: "Execute the active document through /api/run", run: () => runCurrentFile() },
    { id: "debug", label: "Debug current file", description: "Start a debug session with live stepping", run: () => startDebugSession() },
    { id: "build-web", label: "Build web bundle", description: "Export the active file to a web bundle", run: () => buildCurrentFile("web") },
    { id: "build-apk", label: "Build Android scaffold", description: "Export the active file to an Android project", run: () => buildCurrentFile("apk") },
    { id: "save", label: "Save current file", description: "Persist the active tab back to the project", run: () => saveActiveDocument() },
    { id: "save-all", label: "Save all open files", description: "Write all dirty documents to disk", run: () => saveAllDocuments() },
    { id: "new-file", label: "Create new file", description: "Add a file to the current project", run: () => createNewFile() },
    { id: "rename-file", label: "Rename active file", description: "Move the active file within the project tree", run: () => renameActiveFile() },
    { id: "delete-file", label: "Delete active file", description: "Remove the active file from the project", run: () => deleteActiveFile() },
    { id: "split", label: "Toggle split view", description: "Open or close the secondary editor pane", run: () => refs.splitViewBtn.click() },
    { id: "theme", label: "Toggle theme", description: "Switch between dark and light EdgeStudio themes", run: () => toggleTheme() },
    { id: "explain", label: "NEUROEDGE explain", description: "Get a guided explanation of the active file", run: () => runAiAction("explain") },
    { id: "fix", label: "NEUROEDGE review", description: "Ask NEUROEDGE for fixes or improvement hints", run: () => runAiAction("fix") },
    { id: "plugins", label: "Install plugin", description: "Scaffold a local EDGEL plugin", run: () => installPlugin() },
    { id: "logs", label: "Refresh logs", description: "Reload GoldEdge Browser logs in the terminal", run: () => refreshLogs() },
    { id: "explorer", label: "Show Explorer", description: "Switch the sidebar to the file explorer", run: () => switchActivity("explorer") },
    { id: "search", label: "Show Search", description: "Switch the sidebar to project search", run: () => switchActivity("search") },
    { id: "extensions", label: "Show Extensions", description: "Switch the sidebar to plugins", run: () => switchActivity("extensions") },
    { id: "debug-view", label: "Show Debug sidebar", description: "Switch the sidebar to debugger controls", run: () => switchActivity("debug") },
    ...EXAMPLES.map((example) => ({
      id: `example-${example.path}`,
      label: `Open ${example.label}`,
      description: example.path,
      run: () => openProjectFile(example.path)
    }))
  ];
}

function switchActivity(activity) {
  state.activity = activity;
  activityBar.setActive(activity);
  renderSidebarView();
}

function visibleCommands() {
  const query = state.palette.query.trim().toLowerCase();
  return commandDefinitions().filter((command) => {
    if (!query) {
      return true;
    }
    return `${command.label} ${command.description}`.toLowerCase().includes(query);
  });
}

function renderCommandPalette() {
  const commands = visibleCommands();
  refs.paletteResults.innerHTML = commands
    .map(
      (command, index) => `
        <button type="button" class="palette-result ${index === state.palette.index ? "active" : ""}" data-command-id="${command.id}">
          <span class="palette-command">${command.label}</span>
          <span class="palette-description">${command.description}</span>
        </button>
      `
    )
    .join("");

  refs.paletteResults.querySelectorAll("[data-command-id]").forEach((button) => {
    button.addEventListener("click", () => {
      const command = commands.find((entry) => entry.id === button.dataset.commandId);
      closeCommandPalette();
      command?.run();
    });
  });
}

function openCommandPalette() {
  state.palette.open = true;
  refs.palette.hidden = false;
  refs.paletteInput.value = "";
  state.palette.query = "";
  state.palette.index = 0;
  renderCommandPalette();
  refs.paletteInput.focus();
}

function closeCommandPalette() {
  state.palette.open = false;
  refs.palette.hidden = true;
  refs.commandPaletteBtn.focus();
}
