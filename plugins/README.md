# EDGEL Plugins

Create plugins in:

```text
plugins/
└─ plugin-name/
   └─ plugin.egl
```

Supported hooks:

- `onStart(event)`
- `onRun(event)`
- `onBuild(event)`
- `onError(event)`

Use `edgel plugin init <name>` to scaffold a plugin.

