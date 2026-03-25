# EDGEL Package Ecosystem

EDGEL v0.1.0 ships with a local package and plugin foundation designed to grow into a public registry.

## What exists now

- `edgel install <package> [version]`
- `edgel update`
- `edgel publish`
- `edgel.json` for package metadata
- `edgel.lock` for deterministic resolution
- checksum validation for cached packages
- local publish registry under `.edgel/registry/`

## Versioning

- package manifests use semantic versions such as `1.0.0`
- dependency constraints can use ranges such as `^1.0.0`
- lockfiles record the resolved version and checksum

## Plugin distribution

Plugins live in:

```text
plugins/<name>/plugin.egl
```

They can also be packaged and shared like any other EDGEL project once a public registry is published.

## Public registry preparation

The current foundation is ready for:

- signed package uploads
- remote registry indexes
- public package discovery
- versioned plugin distribution

## Recommended public repo

Suggested GitHub repository:

```text
github.com/goldedge-labs/edgel
```

Suggested release download base:

```text
https://github.com/goldedge-labs/edgel/releases/download/v0.1.0/
```
