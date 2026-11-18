# Recipes

One YAML file per app, describing where it keeps its settings, which files to capture, and how to translate them between Windows and macOS.

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
