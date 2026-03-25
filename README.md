# EDGEL

EDGEL is the first build of the GoldEdge Labs programming language ecosystem:

- `edgelvm/`: lexer, parser, IR, bytecode compiler, VM, UI preview renderer, exporters, and local server
- `cli/`: `edgel` command for run, debug, optimize, build, test, profile, info, package flows, serve, repl, and local AI helper flows
- `backend/`: GoldEdge Browser launcher
- `frontend/`: browser IDE for editing, previewing, exporting, and asking NEUROEDGE for guidance
- `database/`: EDGEL data notes and starter schema patterns
- `stdlib/`: high-level EDGEL standard library modules
- `plugins/`: runtime extension entrypoint for `.egl` plugins
- `tooling/`: self-hosted EDGEL tooling scaffolds
- `examples/`: starter EDGEL programs
- `scripts/`: quickstart helpers
- `output/`: generated web and Android export artifacts
- `editor-support/`: initial VS Code language tooling scaffold

## What Works In This Build

- Human-readable EDGEL syntax for apps, screens, text, inputs, buttons, web pages, APIs, models, identity blocks, variables, functions, loops, conditions, and database inserts/queries
- Rust-native lexer, parser, IR lowering, bytecode compilation, and VM execution
- `edgel run`, `edgel debug`, `edgel optimize`, `edgel build`, `edgel build --web`, `edgel build --apk`, `edgel build --bytecode`, `edgel profile`, `edgel info`, `edgel repl`, `edgel ai explain`, `edgel ai fix`, `edgel serve`, `edgel test`, `edgel doctor`, `edgel init`, `edgel install`, `edgel update`, and `edgel publish`
- GoldEdge Browser IDE served locally at `http://127.0.0.1:4040`
- Web export bundle generation
- Android WebView project scaffold export for APK-oriented delivery
- Local NEUROEDGE teaching and syntax guidance layer
- Official source extension: `.egl`
- Import/module loading from `.egl` files plus namespaced imports such as `import std.ui` and `import plugins.logger`
- VM debug/profile support, instruction limits, named `test "..." { ... }` blocks, and multi-error parser recovery
- JSON project/file management API for the browser layer, including profiling and log retrieval
- Plugin discovery, permissions, ordering, hook execution, and shared hook context through `.egl` files
- Standard library imports such as `import ui` and `import std.compiler`
- Partially active self-hosted compiler bridge under `tooling/edgel-compiler/`
- `edgel.lock` generation with checksum validation for installed package caches
- starter templates through `edgel new <name> --template app|web|api`
- interactive learning mode through `edgel learn`
- launch scripts for prebuilt binary packaging and one-command install via `edgel.sh`

## Install

EDGEL is prepared for prebuilt binary distribution in v0.1.0.

```bash
curl -sSL https://edgel.sh | bash
```

Windows PowerShell installer:

```powershell
irm https://github.com/josephwere/EDGEL/releases/download/v0.1.0/edgel.ps1 | iex
```

For local release engineering:

```bash
scripts/release.sh
```

## Quick Start

```bash
cargo run -p edgel -- new my-app --template app
cd my-app
cargo run -p edgel -- run
cargo run -p edgel -- learn
cargo run -p edgel -- run examples/logic.egl
cargo run -p edgel -- build tooling/edgel-compiler/compiler.egl
cargo run -p edgel -- build --web examples/mobile.egl
cargo run -p edgel -- build --apk examples/mobile.egl
cargo run -p edgel -- debug examples/selfhost.egl
cargo run -p edgel -- debug examples/logic.egl --breakpoint 5 --profile
cargo run -p edgel -- optimize examples/selfhost.egl
cargo run -p edgel -- profile examples/selfhost.egl
cargo run -p edgel -- info examples/selfhost.egl
cargo run -p edgel -- ai explain examples/mobile.egl
cargo run -p edgel -- test --report --coverage
cargo run -p edgel -- doctor
cargo run -p edgel -- init demo-app
cargo run -p edgel -- install logger
cargo run -p edgel -- update
cargo run -p edgel -- publish
cargo run -p edgel -- plugin list
cargo run -p edgel -- plugin init logger
cargo run -p goldedge-browser
```

## First User Flow

```bash
edgel new my-app
cd my-app
edgel run
edgel test
edgel learn
```

Templates:

- `edgel new my-app --template app`
- `edgel new my-site --template web`
- `edgel new my-api --template api`

## File Extensions

- `.egl`: official EDGEL source files
- `.eglm`: planned module format
- `.eglc`: planned compiled bytecode format
- `.eglpkg`: planned packaged app format

## Architecture

EDGEL now follows the intended hybrid layering:

```text
[ Rust Core ]
lexer -> parser -> optimizer -> bytecode compiler
        ↓
[ Rust Runtime ]
EdgelVM -> sandbox -> profiling -> plugin host
        ↓
[ High-Level EDGEL ]
stdlib/ -> plugins/ -> tooling/edgel-compiler/ -> user apps
```

1. `lexer.rs` tokenizes EDGEL source.
2. `parser.rs` builds the AST for script, UI, web, API, model, and identity blocks.
3. `ir.rs` lowers executable sections into an intermediate representation.
4. `compiler.rs` turns IR into modular bytecode chunks.
5. `vm.rs` executes bytecode with sandboxed globals, functions, and a tiny in-memory database.
6. `render.rs` converts app and web AST nodes into browser preview documents.
7. `project.rs` powers run, explain, fix, and export flows.
8. `loader.rs` resolves `.egl` imports, namespaced modules, plugin modules, and project entry files.
9. `plugins.rs` discovers `.egl` plugins, applies metadata, permissions, ordering, and shared hook context while keeping Rust in control of safety.
10. `server.rs` serves the GoldEdge Browser frontend, profiling APIs, plugin APIs, and request logs without external dependencies.

## Launch Docs

- [Install Guide](/home/joseph-were/Downloads/EDGEL/docs/install.md)
- [Deployment Guide](/home/joseph-were/Downloads/EDGEL/docs/deploy.md)
- [Beginner Guide](/home/joseph-were/Downloads/EDGEL/docs/beginner-guide.md)
- [First App Tutorial](/home/joseph-were/Downloads/EDGEL/docs/tutorials/first-app.md)
- [Web + API Tutorial](/home/joseph-were/Downloads/EDGEL/docs/tutorials/web-api.md)
- [Package Ecosystem](/home/joseph-were/Downloads/EDGEL/docs/package-ecosystem.md)
- [Release Notes v0.1.0](/home/joseph-were/Downloads/EDGEL/docs/release-notes-v0.1.0.md)

## Project Standard

The production-facing project layout is now supported by `edgel init`:

```text
project/
├─ src/
│  └─ main.egl
├─ tests/
│  └─ basic.test.egl
├─ assets/
├─ config/
│  └─ .edgelconfig
├─ build/
├─ dist/
├─ edgel.json
└─ edgel.lock
```

## Browser / AI API Layer

- `POST /api/run?debug=true&profile=true&path=examples/selfhost.egl`
- `POST /api/profile?path=examples/selfhost.egl`
- `POST /api/debug/start`
- `POST /api/debug/step?session=<id>&action=into|over|out|continue`
- `GET /api/debug/inspect?session=<id>&expr=user.name&frame=0`
- `POST /api/build?target=web`
- `POST /api/build?target=apk`
- `POST /api/build?target=bytecode&path=tooling/edgel-compiler/compiler.egl`
- `GET /api/plugins`
- `GET /api/logs`
- `GET /api/project?action=list`
- `GET /api/project?action=read&path=examples/mobile.egl`
- `POST /api/project?action=write&path=project/src/main.egl`
- `GET /api/project?action=plugins`

If an API request contains relative imports, pass `path=<workspace-relative-file.egl>` so the server can resolve modules exactly like the CLI.

API requests send raw `.egl` source in the request body. Example:

```bash
curl -X POST 'http://127.0.0.1:4040/api/run?debug=true&profile=true&trace=true' \
  --data-binary 'print("hi")'
```

## Deployment (Vercel + Render)

EdgeStudio (frontend) and GoldEdge Browser API (backend) are deployable separately.

Render backend:

- Build & deploy the Docker service with `render.yaml`.
- Set env vars:
  - `EDGEL_ALLOWED_ORIGIN=https://<your-vercel-domain>`
  - `NEUROEDGE_API_URL` (optional)
- Render will inject `PORT`. The backend binds to `0.0.0.0`.

Vercel frontend:

- Import this repo in Vercel.
- Set env var `EDGEL_API_BASE` to your Render service URL (e.g. `https://edgel-api.onrender.com`).
- Build command: `npm run build:frontend`
- Output directory: `dist/vercel`

The frontend uses `frontend/config.js` plus `dist/vercel/config.js` to route API calls to the Render backend.

Production hardening in this phase also adds:

- structured diagnostics from `/api/run`, `/api/profile`, and `/api/build`
- structured debugger sessions from `/api/debug/start`, `/api/debug/step`, and `/api/debug/inspect`
- request logging through `/api/logs`
- API rate limiting at 60 `/api/*` requests per minute per client address
- dependency integrity failures now surface as structured diagnostics when `edgel.lock` and cached packages diverge

## Power Features

- Plugins can declare metadata with `model plugin { name, version, order, permissions, channel }`
- Supported lifecycle hooks now include `onStart`, `onRun`, `onBuild`, `onError`, `onCompile`, `onExecute`, `onApiRequest`, and `onCliCommand`
- Plugin return values are shared forward through `event.plugins.<pluginName>`
- `edgel build <file.egl>` now exports `.eglc` bytecode bundles
- `edgel debug <file.egl> [--breakpoint <line|function:name>] [--profile]` exposes step-into, step-over, step-out, continue, frame selection, local/global inspection, and stack-enriched runtime diagnostics
- `edgel optimize <file.egl>` writes optimized bytecode bundles and reports instruction deltas
- `edgel profile <file.egl>` exposes runtime instructions, builtin calls, stack depth, and elapsed time
- `edgel info <file.egl>` reports summary, functions, tests, instructions, and plugin count
- `edgel new <project-dir> [--template ...]` scaffolds launch-ready starter projects with README, config, tests, and plugin docs
- `edgel learn` exposes built-in lessons, exercises, and debugging guidance from the local docs set
- `edgel test --report --coverage` writes `tests/dist/test-report.json` and prints function-hit coverage data when available
- `edgel install`, `edgel update`, and `edgel publish` manage `edgel.json` dependencies plus `edgel.lock`, checksum-verified caches, and a local package registry foundation
- `edgel doctor` verifies manifest/lock/cache alignment and reports lockfile drift or tampering
- `tooling/edgel-compiler/compiler.egl` can now compile simple snippets through Rust-backed compiler bridge functions
- GoldEdge Browser can now start debugger sessions, step execution, and inspect variables against the selected stack frame
- `scripts/release.sh` packages host or cross-target binaries, and `edgel.sh` installs prebuilt binaries without requiring Rust

NEUROEDGE can now integrate through `NEUROEDGE_API_URL` for HTTP-based assistant calls, with local fallback behavior when no API endpoint is configured.

## Roadmap

This first milestone establishes the language core and integrated developer experience. The next milestones are:

- richer type checking and diagnostics
- remote package registry and signed package distribution
- persistent EDGEL database runtime
- true Android build pipeline with Gradle automation and signing hooks
- LSP server and VS Code extension packaging
- remote-model adapters for full NEUROEDGE cloud intelligence
- IDVerse verification backends
