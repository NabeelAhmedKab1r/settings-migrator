# Settings Migrator

**Clean computer, your settings, any OS.**

An open-source desktop app and command-line tool that moves your app settings to a new computer, including between Windows and Mac. Settings are captured into one portable bundle file, previewed before anything is written, and restored with an undo backup. Secrets (passwords, tokens, session cookies) are never copied.

> **Status:** early development. Capture and restore work from the command line for VS Code, Git, Obsidian, VLC and OBS Studio, on the same OS or between Windows and Mac. The desktop app is coming.

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

Add `--app vscode` (repeatable) to work on some apps only.

Restore shows a diff of every file it would change and asks before writing (`--yes` skips the question). It won't run while an app it changes is open, because an open app can overwrite its settings when it quits. It backs up every file it changes before writing, under `~/Library/Application Support/settings-migrator/undo` on macOS and `%LOCALAPPDATA%\settings-migrator\undo` on Windows. Known secret files (credentials, private keys, cookie and password stores) are never captured. Every other file is scanned before the bundle is written: for known token formats, for any value under a setting named like a password, secret, token or API key (apart from known-safe values such as `osxkeychain` or `${env:NAME}`), and for random-looking strings; if anything is flagged, capture stops and lists it (without printing the secret). Run it again with `--skip APP/FILE` to leave a file out, or `--allow APP/FILE` if you've checked it holds no secret.

### Moving between Windows and Mac

Restore a bundle made on the other OS the same way. Each file is translated as its recipe allows, and the preview lists what changed:

- **Shortcuts:** Ctrl becomes Cmd going to a Mac, and back going to Windows. Alt and Option keep their name, since VS Code calls both `alt`.
- **Paths inside settings:** a path in your old home folder moves to the same place in your new one, such as `C:\Users\Sam\Videos` to `~/Movies`.
- **Line endings:** CRLF on Windows, LF on Mac, for the plain-text config files where the recipe asks for it.

Anything that can't be translated is marked `!` in the preview and restored unchanged, never dropped. Examples are a path outside your home folder (`D:\Vaults`, `/Volumes/Backup`), a shortcut that uses the Windows key, a shortcut the other OS keeps for itself (Cmd+Tab), and files only one OS uses.

## Building

Requires [Rust](https://rustup.rs) 1.80 or later.

```sh
cargo build --workspace
cargo test --workspace
cargo run -p settings-migrator-cli
```

To try commands without touching your real settings, add `--home DIR` (hidden from `--help`, since it's for testing): the folder then stands in for your home folder, and restore and undo skip the open-app check. `crates/engine/tests/fixtures/cross-os/` has sample Windows and Mac home folders to try it on.

## Layout

| Path | What it is |
| --- | --- |
| `crates/engine` | Library that loads recipes and captures, translates and restores settings |
| `crates/cli` | `settings-migrator` command-line tool built on the engine |
| `recipes/` | One YAML recipe per supported app |

## Licence

[Apache 2.0](LICENSE)
