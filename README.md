# Settings Migrator

**Clean computer, your settings, any OS.**

An open-source desktop app and command-line tool that moves your app settings to a new computer, including between Windows and Mac. Settings are captured into one portable bundle file, previewed before anything is written, and restored with an undo backup. Secrets (passwords, tokens, session cookies) are never copied.

> **Status:** early development. Nothing works yet beyond the project skeleton.

## Building

Requires [Rust](https://rustup.rs) 1.80 or later.

```sh
cargo build --workspace
cargo test --workspace
cargo run -p settings-migrator-cli
```

## Layout

| Path | What it is |
| --- | --- |
| `crates/engine` | Library that loads recipes and captures, translates and restores settings |
| `crates/cli` | `settings-migrator` command-line tool built on the engine |
| `recipes/` | One YAML recipe per supported app |

## Licence

[Apache 2.0](LICENSE)
