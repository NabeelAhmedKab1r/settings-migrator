# Settings Migrator

**Clean computer, your settings, any OS.**

An open-source desktop app and command-line tool that moves your app settings to a new computer, including between Windows and Mac. Settings are captured into one portable bundle file, previewed before anything is written, and restored with an undo backup. Secrets (passwords, tokens, session cookies) are never copied.

> **Status:** early development. Same-OS capture and restore work from the command line for VS Code, Git, Obsidian, VLC and OBS Studio. Moving between Windows and Mac, and the desktop app, are coming.

## Using it

```sh
settings-migrator scan                              # what can be captured on this computer
settings-migrator capture -o settings.zip --dry-run  # list the files, write nothing
settings-migrator capture -o settings.zip            # make the bundle
settings-migrator restore settings.zip --dry-run     # see what would change, write nothing
settings-migrator restore settings.zip               # show a diff of every file, ask, back up, restore
settings-migrator undo --dry-run                     # see what undo would put back
settings-migrator undo                               # put back what the last restore changed
```

Add `--app vscode` (repeatable) to work on some apps only, and `--home DIR` to try everything against a test folder instead of your real home.

Restore shows a diff of every file it would change and asks before writing (`--yes` skips the question). It won't run while an app it changes is open, because an open app can overwrite its settings when it quits. It backs up every file it changes before writing, under `~/Library/Application Support/settings-migrator/undo` on macOS and `%LOCALAPPDATA%\settings-migrator\undo` on Windows. Known secret files (credentials, private keys, cookie and password stores) are never captured. Every other file is scanned before the bundle is written: for known token formats, for any value under a setting named like a password, secret, token or API key (apart from known-safe values such as `osxkeychain` or `${env:NAME}`), and for random-looking strings; if anything is flagged, capture stops and lists it (without printing the secret). Run it again with `--skip APP/FILE` to leave a file out, or `--allow APP/FILE` if you've checked it holds no secret.

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
