# Tutorial: First App

## Create the project

```bash
edgel new hello-app --template app
cd hello-app
```

## Run it

```bash
edgel run
```

## Add a button

Edit `src/main.egl`:

```edgel
app HelloApp {
    screen Main {
        header("Hello App")
        text("This is your first EDGEL app.")

        button("Click Me") {
            print("Button clicked")
        }
    }
}
```

## Debug it

```bash
edgel debug src/main.egl --profile
```
