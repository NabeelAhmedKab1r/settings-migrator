# Development

## Building

Requires [Rust](https://rustup.rs) (1.80 or later for the engine and command line, 1.90 for the desktop app) and, for the desktop app, [Node.js](https://nodejs.org) 22 or later.

```sh
cargo build --workspace
SETTINGS_MIGRATOR_HOME="$TMPDIR/settings-migrator-tests" cargo test --workspace
cargo run -p settings-migrator-cli
```

The recipe tests (`crates/engine/tests/recipes.rs`) write fake homes only inside the folder `SETTINGS_MIGRATOR_HOME` names, which must be a new folder in your temp folder; they refuse to run without it. CI sets it to a folder in the runner's temp folder.

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

- **Real-app tests** (`e2e/app`, Windows runner): WebdriverIO and tauri-driver drive the built app through saving, the secret review, the restore preview, restoring and undo, checking the files on disk after each step. It gives up within a few minutes if the app won't start, and uploads the driver, app and WebdriverIO logs as the `real-app-logs-windows` artifact. For now it, and the cross-OS app test, may fail without failing CI.
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
| `recipes/samples/` | Made-up settings for each recipe, which its test saves and restores |
| `contributing/` | Templates for a new recipe and its samples (see [CONTRIBUTING.md](../CONTRIBUTING.md)) |
