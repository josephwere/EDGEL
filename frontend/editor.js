const KEYWORDS = [
  "app",
  "web",
  "screen",
  "page",
  "text",
  "input",
  "button",
  "header",
  "api",
  "db",
  "model",
  "idverse",
  "let",
  "function",
  "return",
  "if",
  "else",
  "for",
  "in",
  "try",
  "catch",
  "import",
  "permissions",
  "table",
  "insert",
  "query",
  "navigate",
  "scene",
  "test",
  "assert",
  "print",
  "alert",
  "fetch",
  "breakpoint"
];

const KEYWORD_SET = new Set(KEYWORDS);

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

function guessLanguage(path) {
  if (path.endsWith(".egl")) {
    return "EDGEL";
  }
  if (path.endsWith(".json")) {
    return "JSON";
  }
  if (path.endsWith(".md")) {
    return "Markdown";
  }
  if (path.endsWith(".js")) {
    return "JavaScript";
  }
  if (path.endsWith(".css")) {
    return "CSS";
  }
  if (path.endsWith(".html")) {
    return "HTML";
  }
  return "Text";
}

function tokenizeLine(line) {
  const tokens = [];
  let index = 0;
  while (index < line.length) {
    const commentStart = line.indexOf("//", index);
    if (commentStart === index) {
      tokens.push({ type: "comment", value: line.slice(index) });
      break;
    }

    const match = line
      .slice(index)
      .match(/^"(?:[^"\\]|\\.)*"|^\d+(?:\.\d+)?|^[A-Za-z_][A-Za-z0-9_.]*|^[^\w\s]/);

    if (!match) {
      tokens.push({ type: "plain", value: line[index] });
      index += 1;
      continue;
    }

    if (commentStart >= index && commentStart < index + match[0].length) {
      tokens.push({ type: "plain", value: line.slice(index, commentStart) });
      tokens.push({ type: "comment", value: line.slice(commentStart) });
      break;
    }

    const value = match[0];
    let type = "plain";
    if (value.startsWith('"')) {
      type = "string";
    } else if (/^\d/.test(value)) {
      type = "number";
    } else if (KEYWORD_SET.has(value)) {
      type = "keyword";
    } else if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value) && line.slice(index + value.length).trimStart().startsWith("(")) {
      type = "function";
    }
    tokens.push({ type, value });
    index += value.length;
  }
  return tokens;
}

function highlightText(content, language) {
  if (language !== "EDGEL") {
    return `${escapeHtml(content)}\n`;
  }

  return (
    content
      .split("\n")
      .map((line) => {
        const tokens = tokenizeLine(line);
        return tokens
          .map((token) => {
            if (token.type === "plain") {
              return escapeHtml(token.value);
            }
            return `<span class="token ${token.type}">${escapeHtml(token.value)}</span>`;
          })
          .join("");
      })
      .join("\n") + "\n"
  );
}

function extractCursor(text, index) {
  const before = text.slice(0, index);
  const lines = before.split("\n");
  return {
    line: lines.length,
    column: lines[lines.length - 1].length + 1
  };
}

function replaceRange(text, start, end, value) {
  return `${text.slice(0, start)}${value}${text.slice(end)}`;
}

export function createEditorWorkbench({
  container,
  onCursorChange,
  onDirtyChange,
  onDocumentChange,
  onBreakpointChange,
  onEmptyAction
}) {
  const state = {
    split: false,
    activePane: "primary",
    documents: new Map(),
    panes: {
      primary: { tabs: [], active: null, refs: null },
      secondary: { tabs: [], active: null, refs: null }
    },
    breakpoints: new Map(),
    suggestions: {
      primary: { visible: false, items: [], index: 0, start: 0, end: 0 },
      secondary: { visible: false, items: [], index: 0, start: 0, end: 0 }
    }
  };

  container.innerHTML = `
    <div class="editor-workbench" id="editorWorkbench">
      ${createPaneMarkup("primary", "Primary Pane")}
      ${createPaneMarkup("secondary", "Secondary Pane")}
    </div>
  `;

  ["primary", "secondary"].forEach((pane) => {
    const root = container.querySelector(`[data-pane-root="${pane}"]`);
    const refs = {
      root,
      tabs: root.querySelector(`[data-pane-tabs="${pane}"]`),
      body: root.querySelector(`[data-pane-body="${pane}"]`),
      empty: root.querySelector(`[data-empty="${pane}"]`),
      gutter: root.querySelector(`[data-gutter="${pane}"]`),
      gutterContent: root.querySelector(`[data-gutter-content="${pane}"]`),
      highlight: root.querySelector(`[data-highlight="${pane}"]`),
      textarea: root.querySelector(`[data-input="${pane}"]`),
      autocomplete: root.querySelector(`[data-autocomplete="${pane}"]`)
    };
    state.panes[pane].refs = refs;
    bindPaneEvents(pane, refs);
  });

  updateLayout();
  renderAll();

  function createPaneMarkup(pane, label) {
    return `
      <section class="editor-pane" data-pane-root="${pane}">
        <div class="editor-pane-head">
          <h2>${label}</h2>
          <div class="tab-strip" data-pane-tabs="${pane}"></div>
        </div>

        <div class="empty-pane" data-empty="${pane}">
          <div class="empty-state">
            <h3>No file open in ${label.toLowerCase()}</h3>
            <p class="muted-text">Open a file from Explorer, use Search, or scaffold a new EDGEL document.</p>
            <div class="empty-actions">
              <button type="button" class="command-button" data-empty-action="new-file">New file</button>
              <button type="button" class="command-button" data-empty-action="open-example">Open mobile example</button>
            </div>
          </div>
        </div>

        <div class="editor-pane-body" data-pane-body="${pane}" hidden>
          <div class="code-gutter" data-gutter="${pane}">
            <div class="gutter-content" data-gutter-content="${pane}"></div>
          </div>
          <div class="code-shell">
            <pre class="code-highlight" data-highlight="${pane}" aria-hidden="true"></pre>
            <textarea class="code-input" data-input="${pane}" spellcheck="false"></textarea>
            <div class="autocomplete-panel" data-autocomplete="${pane}" hidden></div>
          </div>
        </div>
      </section>
    `;
  }

  function bindPaneEvents(pane, refs) {
    refs.root.addEventListener("click", (event) => {
      const tabButton = event.target.closest("[data-open-doc]");
      const closeButton = event.target.closest("[data-close-doc]");
      const breakpointLine = event.target.closest("[data-breakpoint-line]");
      const emptyAction = event.target.closest("[data-empty-action]");

      if (tabButton) {
        event.preventDefault();
        state.activePane = pane;
        state.panes[pane].active = tabButton.dataset.openDoc;
        syncPaneFromDoc(pane);
        renderTabs(pane);
        emitCursor(pane);
        return;
      }

      if (closeButton) {
        event.preventDefault();
        closeDocument(closeButton.dataset.closeDoc, pane);
        return;
      }

      if (breakpointLine) {
        event.preventDefault();
        const line = Number.parseInt(breakpointLine.dataset.breakpointLine ?? "0", 10);
        if (Number.isFinite(line) && line > 0) {
          toggleBreakpointForPane(pane, line);
        }
        return;
      }

      if (emptyAction) {
        event.preventDefault();
        onEmptyAction(emptyAction.dataset.emptyAction ?? "");
      }
    });

    refs.textarea.addEventListener("focus", () => {
      state.activePane = pane;
      emitCursor(pane);
    });

    refs.textarea.addEventListener("click", () => emitCursor(pane));
    refs.textarea.addEventListener("keyup", () => emitCursor(pane));

    refs.textarea.addEventListener("scroll", () => syncScroll(pane));

    refs.textarea.addEventListener("input", () => {
      const doc = currentDoc(pane);
      if (!doc) {
        return;
      }
      doc.content = refs.textarea.value;
      doc.dirty = doc.content !== doc.savedContent;
      refreshDoc(doc.id);
      maybeOpenAutocomplete(pane);
      onDocumentChange(documentSnapshot(doc));
      onDirtyChange(dirtyPaths());
    });

    refs.textarea.addEventListener("keydown", (event) => {
      if (event.key === "Tab") {
        event.preventDefault();
        insertAtCursor(refs.textarea, "    ");
        refs.textarea.dispatchEvent(new Event("input", { bubbles: true }));
        return;
      }

      if (event.key === "Enter" && !event.shiftKey && state.suggestions[pane].visible) {
        event.preventDefault();
        acceptSuggestion(pane);
        return;
      }

      if (event.key === "Enter") {
        event.preventDefault();
        insertIndentedNewline(refs.textarea);
        refs.textarea.dispatchEvent(new Event("input", { bubbles: true }));
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key === " ") {
        event.preventDefault();
        openAutocomplete(pane);
        return;
      }

      if (state.suggestions[pane].visible) {
        if (event.key === "ArrowDown") {
          event.preventDefault();
          moveSuggestion(pane, 1);
        } else if (event.key === "ArrowUp") {
          event.preventDefault();
          moveSuggestion(pane, -1);
        } else if (event.key === "Escape") {
          event.preventDefault();
          hideSuggestions(pane);
        } else if (event.key === "Tab") {
          event.preventDefault();
          acceptSuggestion(pane);
        }
      }
    });
  }

  function insertAtCursor(textarea, text) {
    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    textarea.value = replaceRange(textarea.value, start, end, text);
    const next = start + text.length;
    textarea.selectionStart = next;
    textarea.selectionEnd = next;
  }

  function insertIndentedNewline(textarea) {
    const start = textarea.selectionStart;
    const lineStart = textarea.value.lastIndexOf("\n", start - 1) + 1;
    const currentLine = textarea.value.slice(lineStart, start);
    const indentation = currentLine.match(/^\s*/)?.[0] ?? "";
    insertAtCursor(textarea, `\n${indentation}`);
  }

  function maybeOpenAutocomplete(pane) {
    const refs = state.panes[pane].refs;
    const { word, start, end } = wordAtCursor(refs.textarea.value, refs.textarea.selectionStart);
    if (word.length < 2) {
      hideSuggestions(pane);
      return;
    }
    const items = KEYWORDS.filter((keyword) => keyword.startsWith(word) && keyword !== word);
    if (items.length === 0) {
      hideSuggestions(pane);
      return;
    }
    state.suggestions[pane] = { visible: true, items, index: 0, start, end };
    renderAutocomplete(pane);
  }

  function openAutocomplete(pane) {
    const refs = state.panes[pane].refs;
    const { word, start, end } = wordAtCursor(refs.textarea.value, refs.textarea.selectionStart);
    const items = KEYWORDS.filter((keyword) => keyword.startsWith(word));
    if (items.length === 0) {
      hideSuggestions(pane);
      return;
    }
    state.suggestions[pane] = { visible: true, items, index: 0, start, end };
    renderAutocomplete(pane);
  }

  function hideSuggestions(pane) {
    state.suggestions[pane].visible = false;
    renderAutocomplete(pane);
  }

  function moveSuggestion(pane, direction) {
    const suggestion = state.suggestions[pane];
    suggestion.index = (suggestion.index + direction + suggestion.items.length) % suggestion.items.length;
    renderAutocomplete(pane);
  }

  function acceptSuggestion(pane) {
    const suggestion = state.suggestions[pane];
    const refs = state.panes[pane].refs;
    const value = suggestion.items[suggestion.index];
    refs.textarea.value = replaceRange(
      refs.textarea.value,
      suggestion.start,
      suggestion.end,
      value
    );
    refs.textarea.selectionStart = suggestion.start + value.length;
    refs.textarea.selectionEnd = refs.textarea.selectionStart;
    hideSuggestions(pane);
    refs.textarea.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function renderAutocomplete(pane) {
    const refs = state.panes[pane].refs;
    const suggestion = state.suggestions[pane];
    if (!suggestion.visible || suggestion.items.length === 0) {
      refs.autocomplete.hidden = true;
      refs.autocomplete.innerHTML = "";
      return;
    }

    refs.autocomplete.hidden = false;
    refs.autocomplete.innerHTML = suggestion.items
      .map(
        (item, index) => `
          <button
            type="button"
            class="autocomplete-item ${index === suggestion.index ? "active" : ""}"
            data-suggestion-index="${index}"
          >
            <span class="autocomplete-word">${escapeHtml(item)}</span>
            <span class="autocomplete-detail">EDGEL keyword</span>
          </button>
        `
      )
      .join("");

    refs.autocomplete.querySelectorAll("[data-suggestion-index]").forEach((button) => {
      button.addEventListener("click", () => {
        suggestion.index = Number.parseInt(button.dataset.suggestionIndex ?? "0", 10);
        acceptSuggestion(pane);
      });
    });
  }

  function wordAtCursor(text, cursor) {
    const before = text.slice(0, cursor);
    const match = before.match(/[A-Za-z_][A-Za-z0-9_]*$/);
    if (!match) {
      return { word: "", start: cursor, end: cursor };
    }
    return {
      word: match[0],
      start: cursor - match[0].length,
      end: cursor
    };
  }

  function syncScroll(pane) {
    const refs = state.panes[pane].refs;
    refs.highlight.scrollTop = refs.textarea.scrollTop;
    refs.highlight.scrollLeft = refs.textarea.scrollLeft;
    refs.gutterContent.style.transform = `translateY(-${refs.textarea.scrollTop}px)`;
  }

  function currentDoc(pane = state.activePane) {
    const docId = state.panes[pane].active;
    return docId ? state.documents.get(docId) ?? null : null;
  }

  function renderAll() {
    updateLayout();
    renderTabs("primary");
    renderTabs("secondary");
    syncPaneFromDoc("primary");
    syncPaneFromDoc("secondary");
  }

  function updateLayout() {
    const workbench = container.querySelector("#editorWorkbench");
    workbench.classList.toggle("split", state.split);
    state.panes.secondary.refs.root.hidden = !state.split;
  }

  function syncPaneFromDoc(pane) {
    const refs = state.panes[pane].refs;
    const doc = currentDoc(pane);
    if (!doc) {
      refs.empty.hidden = false;
      refs.body.hidden = true;
      return;
    }

    refs.empty.hidden = true;
    refs.body.hidden = false;
    if (refs.textarea.value !== doc.content) {
      refs.textarea.value = doc.content;
    }
    refs.textarea.dataset.docId = doc.id;
    refs.highlight.innerHTML = highlightText(doc.content, doc.language);
    renderGutter(pane, doc);
    renderAutocomplete(pane);
    syncScroll(pane);
  }

  function renderTabs(pane) {
    const paneState = state.panes[pane];
    paneState.refs.tabs.innerHTML =
      paneState.tabs.length === 0
        ? `<span class="muted-text">No open tabs</span>`
        : paneState.tabs
            .map((docId) => {
              const doc = state.documents.get(docId);
              if (!doc) {
                return "";
              }
              return `
                <button
                  type="button"
                  class="tab-button ${paneState.active === docId ? "active" : ""} ${doc.dirty ? "dirty" : ""}"
                  data-open-doc="${escapeHtml(docId)}"
                >
                  <span class="tab-label">${escapeHtml(doc.label)}</span>
                  <span class="file-meta">${escapeHtml(doc.language)}</span>
                  <span class="tab-close" data-close-doc="${escapeHtml(docId)}">x</span>
                </button>
              `;
            })
            .join("");
  }

  function renderGutter(pane, doc) {
    const refs = state.panes[pane].refs;
    const lines = doc.content.split("\n");
    const breakpoints = state.breakpoints.get(doc.path) ?? new Set();
    refs.gutterContent.innerHTML = lines
      .map(
        (_, index) => `
          <button
            type="button"
            class="gutter-line ${breakpoints.has(index + 1) ? "active-breakpoint" : ""}"
            data-breakpoint-line="${index + 1}"
          >
            <span class="gutter-bullet"></span>
            <span>${index + 1}</span>
          </button>
        `
      )
      .join("");
  }

  function refreshDoc(docId) {
    ["primary", "secondary"].forEach((pane) => {
      const paneState = state.panes[pane];
      if (paneState.active === docId) {
        syncPaneFromDoc(pane);
        emitCursor(pane);
      }
      if (paneState.tabs.includes(docId)) {
        renderTabs(pane);
      }
    });
  }

  function toggleBreakpointForPane(pane, line) {
    const doc = currentDoc(pane);
    if (!doc?.path) {
      return;
    }
    const lines = state.breakpoints.get(doc.path) ?? new Set();
    if (lines.has(line)) {
      lines.delete(line);
    } else {
      lines.add(line);
    }
    state.breakpoints.set(doc.path, lines);
    refreshDoc(doc.id);
    onBreakpointChange(getBreakpointMap());
  }

  function getBreakpointMap() {
    return Array.from(state.breakpoints.entries()).reduce((map, [path, lines]) => {
      map[path] = Array.from(lines).sort((left, right) => left - right);
      return map;
    }, {});
  }

  function emitCursor(pane) {
    const refs = state.panes[pane].refs;
    const doc = currentDoc(pane);
    if (!doc) {
      return;
    }
    const cursor = extractCursor(refs.textarea.value, refs.textarea.selectionStart);
    onCursorChange({
      ...cursor,
      language: doc.language,
      path: doc.path
    });
  }

  function dirtyPaths() {
    return Array.from(state.documents.values())
      .filter((doc) => doc.dirty && doc.path)
      .map((doc) => doc.path);
  }

  function closeDocument(docId, pane = state.activePane) {
    const paneState = state.panes[pane];
    paneState.tabs = paneState.tabs.filter((value) => value !== docId);
    if (paneState.active === docId) {
      paneState.active = paneState.tabs[paneState.tabs.length - 1] ?? null;
    }
    const stillOpen = Object.values(state.panes).some((entry) => entry.tabs.includes(docId));
    if (!stillOpen) {
      state.documents.delete(docId);
    }
    renderAll();
    onDirtyChange(dirtyPaths());
  }

  function documentSnapshot(doc) {
    return {
      id: doc.id,
      path: doc.path,
      label: doc.label,
      content: doc.content,
      dirty: doc.dirty,
      language: doc.language
    };
  }

  return {
    openDocument(document, options = {}) {
      const pane = options.pane ?? state.activePane;
      const id = document.id ?? document.path ?? `${document.label}-${Date.now()}`;
      const existing = state.documents.get(id);
      const next = existing ?? {
        id,
        path: document.path,
        label: document.label ?? basename(document.path ?? id),
        language: document.language ?? guessLanguage(document.path ?? id),
        content: document.content ?? "",
        savedContent: document.content ?? "",
        dirty: false
      };

      next.path = document.path ?? next.path;
      next.label = document.label ?? next.label;
      next.language = document.language ?? next.language;

      if (!existing || !existing.dirty || options.force) {
        next.content = document.content ?? next.content;
        if (options.saved !== false) {
          next.savedContent = next.content;
          next.dirty = false;
        }
      }

      state.documents.set(id, next);
      if (!state.panes[pane].tabs.includes(id)) {
        state.panes[pane].tabs.push(id);
      }
      state.panes[pane].active = id;
      state.activePane = pane;
      if (pane === "secondary") {
        state.split = true;
      }
      renderAll();
      onDirtyChange(dirtyPaths());
      emitCursor(pane);
      return documentSnapshot(next);
    },

    toggleSplit() {
      state.split = !state.split;
      if (state.split && !state.panes.secondary.active) {
        const primaryDoc = currentDoc("primary");
        if (primaryDoc) {
          if (!state.panes.secondary.tabs.includes(primaryDoc.id)) {
            state.panes.secondary.tabs.push(primaryDoc.id);
          }
          state.panes.secondary.active = primaryDoc.id;
        }
      }
      if (!state.split) {
        state.activePane = "primary";
      }
      renderAll();
      return state.split;
    },

    isSplit() {
      return state.split;
    },

    getActiveDocument() {
      const doc = currentDoc();
      return doc ? documentSnapshot(doc) : null;
    },

    getDirtyPaths() {
      return dirtyPaths();
    },

    listDocuments() {
      return Array.from(state.documents.values()).map(documentSnapshot);
    },

    markSaved(pathOrId, content) {
      const doc =
        state.documents.get(pathOrId) ??
        Array.from(state.documents.values()).find((item) => item.path === pathOrId);
      if (!doc) {
        return;
      }
      doc.content = content ?? doc.content;
      doc.savedContent = doc.content;
      doc.dirty = false;
      refreshDoc(doc.id);
      onDirtyChange(dirtyPaths());
    },

    renameDocumentPath(from, to) {
      state.documents.forEach((doc) => {
        if (doc.path === from) {
          doc.path = to;
          doc.label = basename(to);
          doc.language = guessLanguage(to);
          refreshDoc(doc.id);
        }
      });
      onDirtyChange(dirtyPaths());
    },

    removeDocument(path) {
      const doc = Array.from(state.documents.values()).find((item) => item.path === path);
      if (!doc) {
        return;
      }
      closeDocument(doc.id, "primary");
      closeDocument(doc.id, "secondary");
    },

    focusLine(line, pane = state.activePane) {
      const refs = state.panes[pane].refs;
      const doc = currentDoc(pane);
      if (!refs || !doc) {
        return;
      }
      const lines = refs.textarea.value.split("\n");
      const safeLine = Math.max(1, Math.min(line, lines.length));
      const index = lines.slice(0, safeLine - 1).join("\n").length + (safeLine > 1 ? 1 : 0);
      refs.textarea.focus();
      refs.textarea.selectionStart = index;
      refs.textarea.selectionEnd = index;
      emitCursor(pane);
    },

    setSelectedPane(pane) {
      state.activePane = pane;
      emitCursor(pane);
    },

    setBreakpoints(path, lines) {
      state.breakpoints.set(path, new Set(lines));
      const doc = Array.from(state.documents.values()).find((item) => item.path === path);
      if (doc) {
        refreshDoc(doc.id);
      }
      onBreakpointChange(getBreakpointMap());
    },

    getBreakpoints(path) {
      if (!path) {
        return getBreakpointMap();
      }
      return Array.from(state.breakpoints.get(path) ?? []).sort((left, right) => left - right);
    }
  };
}
