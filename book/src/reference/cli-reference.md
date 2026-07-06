# CLI Reference

## spider

```text
spider run [--allow cap[,cap]] <file.sp|project-dir>
spider test [--allow cap[,cap]] [path]
spider new <name>
spider repl [--allow cap[,cap]]
spider fmt [--check] <paths...>
spider check <file.sp>
spider tree <file.sp>
spider tokens <file.sp>
spider explain <CODE>
spider learn <file.sp>
spider lsp
spider --version
```

Exit code `0` means success. Exit code `1` means a user program or project
problem. Exit code `2` means command usage error.

## web

```text
web install <name>
web publish
web audit
web remove <name>
web --version
```

`web` must be run inside a project containing `web.toml`, except for help and
version output.
