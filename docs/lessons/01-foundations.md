# Lesson 1: Foundations

Goal: run a tiny EDGEL program and see readable syntax in action.

Example:

```edgel
print("Hello World")

let age = 20
if age >= 18 {
    print("Adult")
}
```

Exercise:

1. Create a file called `hello.egl`.
2. Print your name.
3. Add a variable and one `if` check.

Try:

```bash
edgel run hello.egl
```

Feedback checklist:

- If you see `expected expression`, check for a missing value after `=`.
- If you see a closing bracket error, check `}` and `)`.
- If the program runs, you should see console output immediately.

Solution:

```edgel
print("Hello Joseph")

let score = 90
if score >= 50 {
    print("Pass")
}
```
