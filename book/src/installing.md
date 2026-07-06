# Installing and Building

Spider is currently built from source with Cargo.

## Requirements

- Rust toolchain with Cargo.
- A terminal.
- This repository.

## Build

```powershell
cargo build --workspace
```

The `spider` executable is produced under `target/debug/spider.exe` on Windows.
The `web` package-manager executable is produced under `target/debug/web.exe`.

## Run Tests

```powershell
cargo test --workspace
```

## PATH

During development, either run the executable by path:

```powershell
.\target\debug\spider.exe --version
```

or add `target/debug` to your `PATH`.

## Updating

Pull the repository, then rebuild:

```powershell
git pull
cargo build --workspace
```

## Troubleshooting

If Cargo cannot find Rust, install the Rust toolchain. If `spider` is not found,
check whether `target/debug` is on `PATH` or run the executable by full path.

## Exercise

Build the workspace and run `spider explain E0301`.

Answer: `E0301` is the divide-by-zero runtime panic code.
