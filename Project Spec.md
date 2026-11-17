# Settings Migrator — Project Spec

Nov 16, 2026 · @Nabeel

## Overview

An open-source desktop app that moves your app settings to a new computer, including between Windows and Mac. Pitch: "Clean computer, your settings, any OS."

**The problem.** Files are easy to move; app setup is not. Browser extensions, editor themes, keyboard shortcuts and terminal setup are lost on every new machine or clean install. Windows has never had a first-party way to migrate third-party app settings, and nothing moves settings across operating systems.

**Who it's for.**

- People switching between Windows and Mac (the main gap)
- People who want a clean install, not a full clone with years of cruft
- Developers with many tools to set up again

**Goals for v1.**

1. Capture settings for the 30 most-used apps on Windows and macOS into one portable file.
2. Restore them on the same OS, or translate them across OSes where the app allows it.
3. Never copy secrets (passwords, tokens, session cookies).
4. Let anyone add support for an app by writing one small recipe file.

## Competitive landscape

Every same-OS move is covered by someone; moving settings between Windows and Mac is covered by no one. That is our angle.

| Tool | Platforms | What it does well | Gap we can fill |
| --- | --- | --- | --- |
| [Endstate](https://github.com/Artexis10/endstate) | Windows only (Mac and Linux announced) | Free, open-source GUI; reinstalls apps via WinGet and Chocolatey; [settings modules for 357 apps](https://windowsnews.ai/article/endstate-brings-settings-backup-to-357-windows-apps-after-a-clean-install.440849) | No Mac yet; no cross-OS; in one [114-app test only about a quarter got settings restored](https://windowsforum.com/windows-news.4/endstate-rebuilds-windows-11-apps-not-full-pc-backups.442176/) |
| [Mackup](https://github.com/lra/mackup) | macOS, Linux | About 15k stars; 400+ app recipes; still maintained (snap updated March 2026) | Command line only; no Windows; syncs by symlink, not a portable file |
| [Windows Backup](https://www.thurrott.com/?p=291774) | Windows | Built in; restores Store apps, some Windows settings, Wi-Fi passwords | No third-party app settings |
| Apple Migration Assistant | macOS | Built in; full Mac-to-Mac move | Clones everything (no clean install); Mac to Windows not supported (from memory, not verified) |

**Our angle:** cross-OS translation (Windows to Mac and back), a friendly app on both platforms, and fewer apps done really well rather than hundreds done partly.

**Biggest threat:** Endstate shipping a good Mac version. It's open source (Apache 2.0), so contributing Mac support to it is a fallback worth keeping in mind (see Risks).

## v1 scope

v1 is settings only, for about 30 apps, on Windows and macOS. Reinstalling apps comes in v2.

**In v1**

- Scan: detect installed apps that have a recipe, and show what can be captured
- Capture: write one portable bundle file (a zip with a manifest)
- Preview: show exactly what will be restored or changed before anything is written
- Restore: same OS, or cross-OS where the recipe supports it
- Undo: back up every file before overwriting it
- Secret guard: block known secret files and scan values for tokens
- Desktop app plus a command-line tool on the same engine

**Out of v1**

- Installing apps (v2, via WinGet on Windows and Homebrew on Mac)
- Linux (v2; Mackup already serves it)
- Cloud sync and accounts: the bundle is just a file the user moves
- Passwords, cookies, browser history, licence keys

**Apps for v1** (all available on both Windows and Mac; to confirm per app while writing recipes)

| Group | Apps | Cross-OS |
| --- | --- | --- |
| Code editors | VS Code, Cursor, Sublime Text, JetBrains IDEs, Zed | Yes; keybindings need Ctrl/Cmd mapping |
| Dev tools | Git config, SSH config (never keys), npm config, Docker Desktop settings | Yes |
| Notes and writing | Obsidian (app settings, not vaults) | Yes |
| Browsers | Firefox, Chrome, Edge, Brave: extension list and bookmarks only | Yes |
| Media and creative | OBS Studio, VLC, Blender, GIMP, Inkscape, Audacity | Mostly; some paths and plugins differ |
| Terminals and shells | Windows Terminal, iTerm2, PowerShell profile, zsh/bash rc files | Partly: themes and fonts map across, shell scripts don't |
| Chat and music | Discord, Spotify, Slack | Limited: mostly synced by the vendor's cloud already |

## Architecture

One engine reads per-app recipe files, captures settings into a portable bundle, translates them for the target OS, and restores them only after the user previews the changes.

&#91;embedded content: how settings move · capture on the old computer, translate and restore on the new\]

Recipes tell each step where an app keeps its settings. The translate step only runs when the two computers use different operating systems.

**Stack**

- Engine and CLI: Rust (single binary per OS, strong file and path handling)
- Desktop app: Tauri with React and TypeScript, reusing the engine directly
- Recipes: YAML files in a `recipes/` folder, one per app
- Tests: GitHub Actions on Windows and macOS runners, with sample config files for each recipe

**Recipe format** (one file per app; the main thing contributors write)

```yaml
id: vscode
name: Visual Studio Code
locations:
  windows: "{appdata}/Code/User"
  macos: "{app_support}/Code/User"
files:
  - path: settings.json
    format: json
  - path: keybindings.json
    format: json
    translate: [modifier-keys]
  - path: snippets/
exclude:
  - globalStorage/
  - workspaceStorage/
extensions:
  list: "code --list-extensions"
  install: "code --install-extension {id}"
cross_os: true
```

**Bundle format.** A `.zip` holding `manifest.json` (format version, source OS, date, apps, a hash per file) plus each file under a logical path such as `vscode/settings.json`. No absolute paths are stored, so a bundle made on Windows can restore on a Mac.

**Cross-OS translation.** Path variables (`{home}`, `{appdata}`, `{app_support}`) resolve per OS. Translators fix what differs: Ctrl/Cmd and Alt/Option in keybindings, absolute paths inside values, line endings, and fonts missing on the target. Anything that can't be translated is flagged in the preview, never silently dropped.

**Secret guard.** Three layers: a denylist of known secret files (cookie and password stores, private keys such as `id_rsa`), a value scan for token patterns and high-entropy strings before writing the bundle, and a review screen listing anything flagged.

**Safe restore.** Check the app is closed, show a diff of every file, back up the originals to an undo folder, then write. One click restores the previous state.

## Build plan

Six phases, each ending in something that works. Give Claude Code one phase at a time, and don't start the next until the "done when" check passes.

1. **Setup.** Repo, Rust workspace, Apache-2.0 licence, README stub, CI on Windows and macOS.
   - Done when: CI is green on both OSes.
2. **Engine core (same OS).** Recipe loader, path variables, scan, capture to bundle, restore with undo backup. CLI commands: `scan`, `capture`, `restore --dry-run`, `restore`, `undo`. First 5 recipes: VS Code, Git, Obsidian, VLC, OBS.
   - Done when: a capture-then-restore round trip passes in CI on both OSes.
3. **Safety.** Secret guard (denylist, value scan, review list) and a file-by-file preview diff.
   - Done when: a test bundle seeded with fake tokens is blocked, and every restore shows its diff first.
4. **Cross-OS translation.** Path rewriting, Ctrl/Cmd mapping, untranslatable items flagged.
   - Done when: CI captures on the Windows runner and restores on the macOS runner (and the reverse) for all 5 recipes.
5. **Desktop app.** Tauri screens: Scan, Choose apps, Capture, Restore preview, Undo.
   - Done when: a non-technical friend moves their settings without help.
6. **30 recipes and contributor kit.** Remaining recipes, a `CONTRIBUTING.md`, a recipe template, and a test-fixture template.
   - Done when: a new recipe can be added and tested in under an hour.

**How to start in Claude Code:** save this doc as `SPEC.md` in an empty folder, then ask Claude Code to "read SPEC.md and do phase 1 only".

## Launch plan

Stars come from the launch, not the code: one strong README, one short video, and posts where switchers and tinkerers already gather.

**Before launch**

- [ ] README: one-line pitch, a GIF of a Windows-to-Mac restore in the first screen, install in one command, supported-apps table
- [ ] 30-second demo video: old Windows laptop, bundle, new Mac, same VS Code setup
- [ ] Signed installers for Windows and macOS (unsigned apps scare non-technical users away)
- [ ] 10 "good first issue" tickets, each a missing app recipe
- [ ] A short write-up: "How I translate app settings between Windows and Mac"

**Launch week** (stagger the posts, one a day)

- [ ] Show HN post with the write-up
- [ ] Reddit: r/macapps, r/windows, r/opensource, r/coolgithubprojects, r/SideProject (the same places Endstate launched)
- [ ] Short clip on TikTok, YouTube Shorts and X
- [ ] Submit to awesome-selfhosted-style lists and open-source alternative directories

**After launch**

- [ ] Answer every issue within a day for the first month
- [ ] Add the most-requested app recipe each week and post the update

## Risks and open questions

The biggest risk is Endstate reaching the Mac first. The fallback is that the skills transfer straight into contributing there.

| Risk | Likelihood | Fallback |
| --- | --- | --- |
| Endstate ships a solid Mac version first | Medium | Lean on cross-OS translation, which it doesn't do; or contribute Mac support to Endstate (Apache 2.0) |
| Cross-OS works for fewer apps than hoped | Medium | Label each recipe "same OS" or "cross-OS" honestly; still useful for clean installs |
| A secret slips into a bundle | Low, high impact | Secret guard plus the review screen; bundles stay local; clear warning in the README |
| App updates change settings formats | High over time | Recipe tests in CI; version ranges in recipes; community fixes |
| Signing costs | Certain | Apple Developer Program is a yearly fee and Windows signing certificates cost extra (check current prices); ship unsigned betas first |

**Open questions**

- [ ] Project name: check GitHub, npm and domains before choosing
- [ ] Rust or Go for the engine? (Rust fits Tauri; Go is quicker to write)
- [ ] Does Apple Migration Assistant already move any app settings from Windows? Test it before claiming the gap in the README
- [ ] Talk to the Endstate developer early? A friendly collaboration could beat a race
