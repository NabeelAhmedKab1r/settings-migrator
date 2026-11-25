<div align="center">

# Settings Migrator

**Clean computer, your settings, any OS.**

Moves your app settings to a new computer, even from Windows to Mac or Mac to Windows.<br>
Free and open source. Nothing leaves your computer.

[![CI](https://github.com/NabeelAhmedKab1r/settings-migrator/actions/workflows/ci.yml/badge.svg)](https://github.com/NabeelAhmedKab1r/settings-migrator/actions/workflows/ci.yml)
[![Licence: Apache 2.0](https://img.shields.io/badge/licence-Apache%202.0-blue.svg)](LICENSE)
![Platforms: Windows | macOS](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS-lightgrey.svg)

<!-- TODO: record docs/demo.gif, a Windows → Mac restore in the desktop app. -->
<img src="docs/demo.gif" alt="Restoring settings saved on Windows onto a Mac" width="720">

</div>

## The problem

Copying your files to a new computer is easy. Getting your apps set up again isn't: your editor's theme and shortcuts, your terminal, your Git setup, your OBS scenes. That takes hours of clicking through settings, and switching between Windows and Mac makes it worse.

Settings Migrator saves those settings into one file and puts them back on the new computer, adjusting them for the new OS as it goes. Ctrl shortcuts become Cmd, and a folder like `C:\Users\you\Videos` becomes `~/Movies`. Anything it can't translate, such as a path on another drive, is flagged in the preview and never quietly dropped.

## Why it's different

| | Settings Migrator | [Endstate](https://github.com/Artexis10/endstate) | [Mackup](https://github.com/lra/mackup) | [Windows Backup](https://support.microsoft.com/help/17128) | [Migration Assistant](https://support.apple.com/102565) |
| --- | :---: | :---: | :---: | :---: | :---: |
| Windows | ✅ | ✅ | ❌ | ✅ | ❌ ¹ |
| macOS | ✅ | ❌ | ✅ | ❌ | ✅ |
| Windows ↔ Mac | ✅ | ❌ | ❌ | ❌ | Partly ¹ |
| Third-party app settings | ✅ | ✅ | ✅ | ❌ ² | Mac to Mac |
| Clean install (not a clone) | ✅ | ✅ | ✅ | ✅ | ❌ ³ |
| Desktop app | ✅ | ✅ | ❌ | ✅ | ✅ |

¹ Migration Assistant brings files and some settings (such as email accounts, contacts and bookmarks) from a Windows PC to a Mac, one way only.<br>
² Windows Backup saves Windows' own settings and your list of apps, not the apps' settings.<br>
³ Between Macs, it [copies your whole user account, apps and settings](https://support.apple.com/102613) from the old Mac.

<sub>As of November 2026. Corrections welcome: [open an issue](https://github.com/NabeelAhmedKab1r/settings-migrator/issues).</sub>

## How it works

1. **Save.** On your old computer, choose *Save settings from this computer*. Pick the apps to bring, and it saves one settings file.
2. **Move.** Copy that file to your new computer: a USB stick, a shared folder, anything.
3. **Preview.** Choose *Put settings on this computer* and pick the file. You see every change, file by file, before anything is written, including what was adjusted for the new OS.
4. **Restore.** Your current settings are backed up first. Changed your mind? *Undo* puts everything back as it was.

## Your secrets stay secret

- **Never copied:** passwords, tokens, cookies, sign-in data and private keys. Known secret files, like SSH keys and browser password stores, are blocked whatever an app's recipe says.
- **Scanned first:** every other file is checked for anything that looks like a password or key. If a file is flagged, you decide, and leaving it out is the default.
- **No account, no cloud:** the settings file is saved on your computer, and you move it yourself. Nothing is uploaded anywhere.

## Supported apps (30)

| Group | Apps | Windows ↔ Mac |
| --- | --- | :---: |
| Code editors | VS Code, Cursor, Sublime Text, JetBrains IDEs, Zed | ✅ |
| Dev tools | Git, SSH config (never keys), npm config | ✅ |
| | Docker Desktop | Same OS only |
| Notes | Obsidian (app settings, not your vaults) | ✅ |
| Browsers | Firefox, Chrome, Edge, Brave (bookmarks only) | ✅ |
| Media and creative | OBS Studio, VLC, Blender, GIMP, Inkscape, Audacity | ✅ |
| Terminals | WezTerm, Alacritty, Starship prompt | ✅ |
| | Windows Terminal (Windows), iTerm2 (Mac) | One OS only |
| Shells | PowerShell profile, bash, zsh (Mac) | Same OS only |
| Text editors | Vim, Neovim | ✅ |

A few notes:

- Shortcuts switch between Ctrl and Cmd for VS Code and Cursor. Sublime Text keeps a separate shortcut file for each OS. JetBrains, Zed and the other apps' shortcuts are copied as they are.
- After restoring Firefox, open it and choose *Bookmarks → Manage bookmarks → Import and Backup → Restore* to load your bookmarks.

Each app's recipe in [`recipes/`](recipes) says exactly which files are saved and which are left out. Want another app? A new recipe takes under an hour: see [CONTRIBUTING.md](CONTRIBUTING.md), or pick a [good first issue](https://github.com/NabeelAhmedKab1r/settings-migrator/labels/good%20first%20issue).

## Install

> **Early release.** It always backs up before changing anything and can undo every restore, but try it with a few apps first, and please [report anything odd](https://github.com/NabeelAhmedKab1r/settings-migrator/issues).

Installers for Windows and macOS are coming with the first release. Until then, you can [build it from source](docs/DEVELOPMENT.md#building).

## Command line

Everything the desktop app does also works from a terminal:

```sh
settings-migrator scan                             # which apps have settings on this computer
settings-migrator capture -o settings.zip          # save them into one file
settings-migrator restore settings.zip --dry-run   # see what would change, write nothing
settings-migrator restore settings.zip             # show every change, ask, back up, restore
settings-migrator undo                             # put back what the last restore changed
```

Add `--app vscode` (repeatable) to work on some apps only, and when capture flags a possible secret, run it again with `--skip APP/FILE` to leave that file out or `--allow APP/FILE` if you've checked it holds no secret.

<details>
<summary><b>What changes between Windows and Mac</b></summary>

Restore a file made on the other OS the same way. Each file is translated as its recipe allows, and the preview lists what changed:

- **Shortcuts:** Ctrl becomes Cmd going to a Mac, and back going to Windows (VS Code and Cursor). Alt and Option keep their name, since VS Code calls both `alt`.
- **Paths inside settings:** a path in your old home folder moves to the same place in your new one, such as `C:\Users\Sam\Videos` to `~/Movies`.
- **Line endings:** CRLF on Windows, LF on Mac, for the plain-text config files where the recipe asks for it.

Anything that can't be translated is marked `!` in the preview and restored unchanged, never dropped. Examples are a path outside your home folder (`D:\Vaults`, `/Volumes/Backup`), a shortcut that uses the Windows key, and a shortcut the other OS keeps for itself (Cmd+Tab). Settings that each OS usually sets differently, such as Git's `core.autocrlf`, are marked too, so you can check them. Files only one OS uses are listed as skipped, with the reason.

</details>

## FAQ

<details>
<summary><b>Why not Discord, Spotify or Slack?</b></summary>

Their settings sync through your account, so they come back when you sign in. What they keep on disk is mostly sign-in state, which Settings Migrator never copies.

</details>

<details>
<summary><b>Does it install my apps too?</b></summary>

Not yet. Install the apps on the new computer, then restore their settings. Installing apps for you (with WinGet on Windows and Homebrew on Mac) is planned.

</details>

<details>
<summary><b>Is it safe to use on my real computer?</b></summary>

It's built so nothing changes without you seeing it first:

- **Preview:** every change is shown file by file before anything is written.
- **Open apps:** it won't restore while an app it would change is open, because an open app can overwrite its settings when it quits.
- **Backup:** every file it changes is backed up first.
- **Undo:** one click (or `settings-migrator undo`) puts everything back as it was.

</details>

<details>
<summary><b>What about Linux?</b></summary>

Linux is planned. Until then, [Mackup](https://github.com/lra/mackup) works well there.

</details>

## Contributing

New app recipes are the most useful contribution, and the guide in [CONTRIBUTING.md](CONTRIBUTING.md) walks you through one in under an hour. To build, test or change the app itself, see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Licence

[Apache 2.0](LICENSE)
