# Tutorial: Web + API

## Create the project

```bash
edgel new my-site --template web
cd my-site
```

## Run and build

```bash
edgel run
edgel build --web
```

## Starter file

```edgel
web MySite {
    page "/" {
        h1("Welcome")
        p("Powered by EDGEL")
    }

    api "/health" {
        return { status: "ok", version: "0.1.0" }
    }
}
```

## Next step

Use `edgel ai explain src/main.egl` if you want a guided walkthrough.
