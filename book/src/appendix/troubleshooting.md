# Troubleshooting

## `spider: build is not a command`

`spider build` is not implemented yet.

## E0001 Tabs in indentation

Use spaces for indentation.

## E0244 Missing capability

Add the capability to `web.toml` or pass `--allow` for a script run.

## E0303 List position

List indexes start at 0. Check the list length before indexing.

## Package install refused

The dependency asks for capabilities your project does not allow. Add the
capability only if you trust the package.
