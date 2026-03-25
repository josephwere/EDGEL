# Editor Support

This repository now includes a lightweight VS Code language package scaffold in `editor-support/vscode/`.

## Included

- `.egl` as the default file association
- legacy `.edgel` recognition for compatibility
- line comments
- bracket pairing
- TextMate grammar scaffold for EDGEL keywords, strings, numbers, and calls

## Next Step

- add semantic tokens
- package to `.vsix`
- connect an EDGEL LSP server to diagnostics, completions, and hover docs
- map the same grammar rules into JetBrains tooling
