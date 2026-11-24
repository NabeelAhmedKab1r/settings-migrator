# Recipes

One YAML file per app, describing where it keeps its settings, which files to capture, and how to translate them between Windows and macOS. To add one, start from [`contributing/recipe-template.yaml`](../contributing/recipe-template.yaml) and follow [CONTRIBUTING.md](../CONTRIBUTING.md). Each recipe has made-up sample settings in `samples/<id>/`, which `crates/engine/tests/recipes.rs` saves and restores.

## Folder names that differ between computers

One folder name in a `files` path may use `*`, for apps that name a folder after their version (`IntelliJIdea2025.2`, Blender's `4.3`) or give it a random name (a Firefox profile):

```yaml
files:
  - path: "*/options/"
  - path: "Profiles/*.default-release/user.js"
```

Saving keeps each matching folder's name, but only the newest version of each product (`IntelliJIdea2025.2`, not an older `IntelliJIdea2024.3` left behind by an update). Restoring puts the files into the same folder if it exists, or else the newest version of the same product here (a warning shows if that's older than the one saved from), or else the only matching folder. If there's no matching folder, or several and none clearly right, the file is skipped and the preview says why: usually, open the app once first so it makes its folder. `*` can't be used in a file name or in more than one folder name.

## Translating

Each entry under `files` can list `translate` steps for restoring on the other OS:

| Step | What it does |
| --- | --- |
| `modifier-keys` | Ctrl ↔ Cmd in VS Code-style keybinding files (`"key": "ctrl+shift+p"`) |
| `absolute-paths` | Moves paths in the old home folder to the new one; flags any other path |
| `line-endings` | CRLF on Windows, LF on macOS. Only for files the app reads as plain text; leave it out for files an app checksums |
| `fonts` | Reserved; not translated yet |

A file only one OS uses gets `os: windows` or `os: macos`, and is skipped on the other. Set `cross_os: false` for an app whose settings can't move between OSes at all.

A setting that Windows and macOS usually set differently goes under `os_specific`, as `section.key` with a note on why (`{to}` stands for the OS being restored on). Restoring on the other OS flags it in the preview, with its line, and leaves its value alone:

```yaml
  - path: .gitconfig
    format: ini
    os_specific:
      core.autocrlf: "sets how Git changes line endings, ...; check it suits {to}"
```

This works for INI-style files such as Git config; section and key names match ignoring case.
