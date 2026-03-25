# Self-Hosted Compiler Bridge

This folder represents the high-level EDGEL compiler track.

Rust remains the production fallback and core implementation for:

- lexing
- parsing
- optimization
- bytecode generation
- VM execution

The `.egl` files here now provide a partial bridge layer:

- `lexer.egl` calls the Rust token bridge
- `parser.egl` calls the Rust IR bridge
- `ir.egl` calls the Rust bytecode bridge
- `compiler.egl` composes those stages into a usable EDGEL-side compiler report

Today this means EDGEL can orchestrate compilation for simple programs while Rust still owns the production compiler core.
