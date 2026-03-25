# Lesson 3: Web + API

Goal: combine a web page and an API in one `.egl` file.

Example:

```edgel
web LaunchSite {
    page "/" {
        h1("Welcome")
        p("Powered by EDGEL")
    }

    api "/health" {
        return { status: "ok" }
    }
}
```

Exercise:

1. Change the heading text.
2. Add one more field to the API response.

Try:

```bash
edgel run src/main.egl
edgel build --web src/main.egl
```

Feedback checklist:

- `page` blocks describe preview HTML.
- `api` blocks return structured objects.
- Keep string values in quotes.

Solution:

```edgel
web LaunchSite {
    page "/" {
        h1("Hello from GoldEdge")
        p("Powered by EDGEL")
    }

    api "/health" {
        return { status: "ok", version: "0.1.0" }
    }
}
```
