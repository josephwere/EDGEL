# EDGEL Beginner Guide

EDGEL is a readable programming language for apps, APIs, sites, and automation.

## First five minutes

```bash
edgel new my-app
cd my-app
edgel run
edgel test
```

## Choose a starter template

- `edgel new my-app --template app`
- `edgel new my-site --template web`
- `edgel new my-api --template api`

## Learn by doing

- `edgel learn`
- `edgel ai explain src/main.egl`
- `edgel debug src/main.egl --profile`

## Build outputs

- `edgel build --web`
- `edgel build --bytecode`
- `edgel build --apk`

## GoldEdge Browser

Run the local IDE with:

```bash
goldedge-browser
```

Or from the source tree:

```bash
cargo run -p goldedge-browser
```
