# EDGEL Database Notes

This folder documents the current MVP data model approach.

## Current Runtime

- `insert` writes to an in-memory table map inside the VM
- `query` returns rows from that in-memory table map
- `table` declarations describe schemas for tooling, validation, and future persistence layers

## Example

```edgel
db connect "school"

table students {
    id: number
    name: text
    course: text
}

insert students { id: 1, name: "Alice", course: "CS" }
query students where true
```

## Next Step

The next persistence layer can map these declarations to SQLite, Postgres, or an EDGEL-native file engine without changing application syntax.

