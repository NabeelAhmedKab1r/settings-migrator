# Contributing

Thanks for helping. The most useful contribution is a **recipe**: one small YAML file that tells Settings Migrator where an app keeps its settings. With the templates below, adding and testing a recipe takes under an hour.

Bug reports and fixes to the engine, command line or desktop app are welcome too. Open an issue first for anything bigger than a small fix.

## Ground rules

- **Never copy secrets.** A recipe lists settings files only. Leave out anything that can hold a password, token, cookie, session, licence key or private key, even if it also holds settings. The engine blocks known secret files and scans every file, but the recipe is the first line of defence.
- **Settings, not data.** No history, caches, logs, downloaded plugins, or the user's own documents (an Obsidian vault, a Firefox history database).
- **Be honest about cross-OS.** Set `cross_os: true` only if the settings work on the other OS once translated. Shell scripts and OS-specific apps are `cross_os: false`.
- **Tests use fake files only.** Sample settings are made up, and tests write only inside a temp folder (see [Testing](#4-test-it-10-minutes)).

## Add a recipe in under an hour

### 1. Find the settings (15 minutes)

On each OS the app runs on, find the folder where it keeps its settings, and which files in it are settings. Good sources: the app's documentation ("where are my settings stored"), its "open settings file" command, or looking in these folders after changing a setting:

| Variable | Windows | macOS |
| --- | --- | --- |
| `{home}` | `C:\Users\<you>` | `/Users/<you>` |
| `{appdata}` | `C:\Users\<you>\AppData\Roaming` | |
| `{localappdata}` | `C:\Users\<you>\AppData\Local` | |
| `{app_support}` | | `~/Library/Application Support` |
| `{preferences}` | | `~/Library/Preferences` |

Note anything that looks secret (a `token`, `auth` or `session` file, a login database) so you can exclude it.

### 2. Write the recipe (15 minutes)

```sh
cp contributing/recipe-template.yaml recipes/<id>.yaml
```

Fill it in. The template explains every field, and [`recipes/README.md`](recipes/README.md) lists the translation steps and the less common options. The existing recipes are good examples:

| If the app… | Look at |
| --- | --- |
| keeps a JSON settings folder | `vscode.yaml`, `cursor.yaml` |
| uses INI files | `git.yaml`, `vlc.yaml` |
| keeps settings in a folder named after its version | `jetbrains.yaml`, `blender.yaml` (`*` in a folder name) |
| keeps settings in a profile folder with a random name | `firefox.yaml` |
| is on one OS only | `windows-terminal.yaml`, `iterm2.yaml` |
| keeps a stream key or token next to its settings | `obs.yaml` (`exclude`) |

### 3. Add sample settings (10 minutes)

```sh
cp -r contributing/sample-template recipes/samples/<id>
```

In `recipes/samples/<id>/windows/` and `macos/`, put small, made-up versions of the app's settings files, laid out as they are inside the recipe's location on that OS. Delete the folder for an OS the recipe doesn't support.

Also add a file or two the recipe must **not** save, such as a cache, a log or an empty fake `Cookies` file, and list them under `not_saved` in `sample.yaml`. The test then proves they're left out.

Keep the samples free of anything that looks like a secret (long random strings, settings named `password` or `token`). If you deliberately include a fake secret to check the secret guard catches it, list that file under `flagged`.

### 4. Test it (10 minutes)

The test puts your samples in a fake home folder, saves them, restores them on a fresh fake home, and, for a cross-OS recipe, plans restoring them on the other OS. It checks:

- exactly the right files are saved (every sample file except `not_saved` and `flagged`);
- every saved file comes back byte for byte on the same OS;
- moving to the other OS skips nothing it shouldn't;
- nothing in the samples looks like a secret, unless it's listed under `flagged`.

**In GitHub Actions (simplest):** push your branch to your fork. The `Test` jobs run it on Windows and macOS.

**On your own computer:** the test only writes inside the folder `SETTINGS_MIGRATOR_HOME` names, which must be a new folder in your temp folder; it refuses to run otherwise.

```sh
# macOS
SETTINGS_MIGRATOR_HOME="$TMPDIR/settings-migrator-tests" SETTINGS_MIGRATOR_RECIPE=<id> \
  cargo test -p settings-migrator-engine --test recipes
```

```powershell
# Windows (PowerShell)
$env:SETTINGS_MIGRATOR_HOME = "$env:TEMP\settings-migrator-tests"
$env:SETTINGS_MIGRATOR_RECIPE = "<id>"
cargo test -p settings-migrator-engine --test recipes
```

A failure says what to fix: a file saved that shouldn't be (add it to `exclude`), a file not saved (check `files`), or something flagged as a secret.

### 5. Try it for real, if you have the app (optional)

`cargo run -p settings-migrator-cli -- scan --app <id>` shows what the recipe finds in your real settings, and `capture --app <id> -o test.zip --dry-run` lists the files it would save without writing anything. Check nothing in that list could hold a secret.

### 6. Open a pull request (5 minutes)

Before opening it, run what CI runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

In the description, say which app versions you checked the paths against, on which OS, and anything the recipe leaves out on purpose.

## Changing the engine, command line or app

The layout is in the [development guide](docs/DEVELOPMENT.md#layout). Run the checks above, plus `npm test` and `npm run build` in `app/` for front-end changes. Every command that writes files needs a dry-run option, and every restore must back up what it replaces first.
