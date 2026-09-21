# codemap

Local-first code search for Rust projects. Ask questions about a codebase
and get file and line answers, with no cloud and no data leaving your machine.

## Usage

```bash
codemap list ./project          # list all functions
codemap callers ./project name  # who calls this function?
codemap ask ./project "cancel token"   # search by keywords
```

## Performance

| Project | Functions | Time |
|---|---|---|
| tokio-util | 619 | 240 ms |

## Roadmap

- [x] Function extraction with tree-sitter
- [x] Call graph (`callers`)
- [x] Keyword search (`ask`)
- [ ] Semantic search with local embeddings
- [ ] File watcher for auto-reindex
- [ ] MCP server and VS Code extension

## Known limitations

`callers` matches by function name only, not by type.