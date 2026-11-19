# Settings Migrator

**Clean computer, your settings, any OS.**

An open-source desktop app and command-line tool that moves your app settings to a new computer, including between Windows and Mac. Settings are captured into one portable bundle file, previewed before anything is written, and restored with an undo backup. Secrets (passwords, tokens, session cookies) are never copied.

> **Status:** early development. Capture and restore work for VS Code, Git, Obsidian, VLC and OBS Studio, on the same OS or between Windows and Mac, in the desktop app or from the command line. There are no installers yet; build it from source (below).

## The desktop app

1. On your old computer, choose **Save settings from this computer**. The app finds your apps, you pick which ones to bring, and it saves one settings file. If a file looks like it holds a password or key, it asks first; leaving the file out is the default.
2. Copy that file to your new computer.
3. There, choose **Put settings on this computer** and pick the file. You see every change file by file before anything is written, including what was adjusted for Windows or Mac and anything that couldn't be. Your current settings are backed up first.
4. Changed your mind? **Undo** puts everything back as it was.

## The command line

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

Anything that can't be translated is marked `!` in the preview and restored unchanged, never dropped. Examples are a path outside your home folder (`D:\Vaults`, `/Volumes/Backup`), a shortcut that uses the Windows key, a shortcut the other OS keeps for itself (Cmd+Tab), and files only one OS uses. Settings that each OS usually sets differently, such as Git's `core.autocrlf`, are marked too, so you can check them.

## Building

Requires [Rust](https://rustup.rs) (1.80 or later for the engine and command line, 1.90 for the desktop app) and, for the desktop app, [Node.js](https://nodejs.org) 22 or later.

```sh
cargo build --workspace
cargo test --workspace
cargo run -p settings-migrator-cli
```

The desktop app is a [Tauri](https://tauri.app) app with a React and TypeScript front end, in `app/`:

```sh
cd app
npm ci
npm run tauri dev                          # run it, reloading as you edit
npm test                                   # front-end tests
npm run tauri build                        # build installers into target/release/bundle
```

### Tests that run only in GitHub Actions

`app/e2e/` holds tests that drive the app and write settings files, so they run only in CI, never on your own computer:

- **Real-app tests** (`e2e/app`, Windows runner): WebdriverIO and tauri-driver drive the built app through saving, the secret review, the restore preview, restoring and undo, checking the files on disk after each step.
- **UI tests** (`e2e/ui`, both runners): Playwright goes through every screen and error state (an app left open, a possible secret, files changed since the preview), with the engine scripted. Screenshots of every screen are uploaded as the `ui-screenshots-webkit` and `ui-screenshots-chromium` artifacts.
- **Cross-OS app test:** the settings file the real app saves on Windows is restored with the command-line tool on macOS and checked file by file.

Each refuses to start unless it's running in GitHub Actions with `SETTINGS_MIGRATOR_HOME` set to a folder inside the runner's temp folder that isn't, and doesn't hold, a real home folder (`app/e2e/guard.ts`).

To try things without touching your real settings, give it a stand-in home folder: `--home DIR` on the command line (hidden from `--help`, since it's for testing), or `SETTINGS_MIGRATOR_HOME=DIR` for the desktop app, which then shows a test-mode banner. Restore and undo then skip the open-app check. `crates/engine/tests/fixtures/cross-os/` has sample Windows and Mac home folders to try it on.

## Layout

| Path | What it is |
| --- | --- |
| `crates/engine` | Library that loads recipes and captures, translates and restores settings |
| `crates/cli` | `settings-migrator` command-line tool built on the engine |
| `app/` | Desktop app: React screens in `app/src`, Tauri commands calling the engine in `app/src-tauri` |
| `recipes/` | One YAML recipe per supported app |

## Licence

[Apache 2.0](LICENSE)
