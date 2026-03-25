# Lesson 4: Debugging

Goal: use the EDGEL debugger to inspect state instead of guessing.

Example:

```edgel
let user = { name: "Asha" }
breakpoint()
print(user.name)
```

Try:

```bash
edgel debug app.egl --profile
```

Useful debugger commands:

- `step`
- `next`
- `out`
- `continue`
- `stack`
- `locals`
- `globals`
- `print user.name`

Feedback checklist:

- Use `breakpoint()` when you want the debugger to pause exactly there.
- Use `--breakpoint 12` or `--breakpoint function:main` when you do not want to edit source.
- If a value is `null`, you are likely paused before it was assigned.

Solution pattern:

1. Start the debugger.
2. Run `locals`.
3. Run `print user.name`.
4. Continue after confirming the value.
