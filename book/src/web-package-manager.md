# web Package Manager

`web` is Spider's package manager. The current implementation is an M5 local
registry, not a public network registry.

## Commands

| Command | Purpose |
| --- | --- |
| `web install <name>` | Install latest local-registry version into `web_modules`. |
| `web publish` | Publish the current project to the local registry. |
| `web audit` | Check installed packages, capabilities, and fingerprints. |
| `web remove <name>` | Remove an installed package and lockfile entry. |
| `web --version` | Show the package manager version. |

## Manifest

`web.toml` records project metadata, capabilities, and dependencies:

```toml
[project]
name = "demo"
version = "0.1.0"
spider = "0.1"

[capabilities]
allow = []

[dependencies]
```

## Capability Diffing

Installation refuses a package whose capabilities exceed the project allow-list.
This is the security center of M5. A dependency that wants `fs` cannot be
installed into a project whose manifest allows no capabilities.

## Lockfile

`web.lock` records exact package versions and deterministic FNV-64 content
fingerprints. The fingerprint is an integrity check for local tampering, not a
cryptographic signature.

## Limitations

- Registry is local: `~/.spider/registry` or `SPIDER_REGISTRY`.
- Published versions are immutable.
- Transitive package dependencies are not resolved.
- Public registry, signing, namespace ownership, semver enforcement, and
  vulnerability feeds are future work.

## Exercise

Why are install scripts omitted?

Answer: Spider's package model treats packages as data. Arbitrary install-time
code would undermine deterministic builds and capability security.
