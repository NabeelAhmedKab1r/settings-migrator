# Cross-OS sample settings

Fake settings for every recipe, used to test moving settings between Windows and macOS. Nothing here is real.

| Folder | What it is |
| --- | --- |
| `windows-home/` | A home folder laid out as on Windows (`AppData/Roaming/...`) |
| `macos-home/` | A home folder laid out as on macOS (`Library/Application Support/...`) |
| `expected-on-macos/` | Each app's files after restoring `windows-home` on a Mac, by app id and recipe path |
| `expected-on-windows/` | The same for restoring `macos-home` on Windows |

In the expected files, `{home}` stands for the home folder restored into, written with forward slashes (as paths are written into Windows text files that aren't JSON), and `{home_json}` for the same folder as written into JSON: with native separators, escaped. Files keep their exact bytes, line endings included (see `.gitattributes`).

CI captures `windows-home` on the Windows runner and restores it on the macOS runner, and the reverse; `cargo test` does both on any OS.
