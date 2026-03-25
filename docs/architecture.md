# EDGEL Architecture

## Core Runtime

- Lexer: converts source text into EDGEL tokens
- Parser: converts tokens into AST structures for scripts, apps, web pages, APIs, AI blocks, and identity blocks, with multi-error recovery and friendlier missing-token guidance
- IR: extracts executable statements and named functions
- Compiler: lowers IR into chunk-based bytecode instructions and now exposes optimized and unoptimized compilation paths
- VM: executes bytecode in a controlled environment with debug tracing, step-recorded snapshots, function-hit profiling, and stack-enriched runtime errors
- Plugin Host: runs `.egl` hook functions while Rust preserves safety and isolation

## Delivery Layers

- CLI: local automation, debugging, optimization, package, and export workflows
- GoldEdge Browser: browser IDE served locally
- Exporters: web bundle and Android WebView scaffold
- NEUROEDGE: local teacher/debug assistant interface
- Standard Library: high-level `.egl` modules imported through the loader
- Self-Hosted Track: `.egl` compiler scaffolds under `tooling/edgel-compiler/`
- Package Layer: `edgel.json`, `edgel.lock`, checksum-verified local package cache, and local publish registry scaffold
- API Layer: raw-source execution/build endpoints, structured diagnostics, debugger session endpoints, request logging, and basic rate limiting

## Design Principle

Keep the language readable, keep the runtime portable, and keep the developer path unified across beginner and professional use cases.
