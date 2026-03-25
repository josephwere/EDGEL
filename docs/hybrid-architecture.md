# EDGEL Hybrid Architecture

## Foundation Rule

EDGEL grows upward:

```text
[ Low-Level Core (Rust) ]
        ↓
[ Virtual Machine / Runtime (Rust) ]
        ↓
[ High-Level Language + Ecosystem (.egl) ]
```

Rust remains responsible for:

- parsing
- optimization
- bytecode generation
- VM execution
- memory safety
- sandboxing

EDGEL grows into:

- standard library modules
- plugins
- build automation
- framework logic
- self-hosted compiler layers
- profiling and info workflows
- package and manifest orchestration
- API-facing orchestration layers

## Current Top-Level Layers

- `edgelvm/`: stable core and runtime host
- `cli/`: operational shell around the Rust engine
- `stdlib/`: reusable `.egl` surface modules
- `plugins/`: extension entrypoint with metadata, permissions, order, and hook lifecycle
- `tooling/edgel-compiler/`: self-hosted compiler bridge
- `edgel.json` + `edgel.lock` + `packages/`: local dependency, lock, and package foundation
- `frontend/` and `backend/`: API-coupled GoldEdge Browser integration, including debugger session control and live variable inspection

## Principle

EDGEL is built on Rust, not a replacement for Rust.
