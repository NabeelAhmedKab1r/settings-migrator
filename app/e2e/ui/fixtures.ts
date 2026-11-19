// What the engine's commands return, for the UI tests. Shaped exactly like
// the Rust structs in app/src-tauri/src/commands.rs (via src/api.ts), with
// values like a real Windows-to-Mac move of the sample settings.

import type {
  AppInfo,
  CaptureFile,
  RestoreFile,
  RestorePreview,
  ScannedApp,
  UndoPreview,
} from "../../src/api";

const HOME = "/Users/sam";
const SUPPORT = `${HOME}/Library/Application Support`;

export const appInfo: AppInfo = {
  os: "macOS",
  test_home: null,
  test_save_as: null,
  test_open: null,
};

export const scan: { apps: ScannedApp[] } = {
  apps: [
    {
      id: "git",
      name: "Git",
      status: "found",
      folder: HOME,
      files: [
        { path: ".config/git/ignore", size: 18 },
        { path: ".gitconfig", size: 210 },
      ],
    },
    {
      id: "obs",
      name: "OBS Studio",
      status: "found",
      folder: `${SUPPORT}/obs-studio`,
      files: [
        { path: "basic/profiles/Main/basic.ini", size: 120 },
        { path: "basic/scenes/Main.json", size: 900 },
        { path: "global.ini", size: 30 },
        { path: "user.ini", size: 40 },
      ],
    },
    {
      id: "obsidian",
      name: "Obsidian",
      status: "found",
      folder: `${SUPPORT}/obsidian`,
      files: [
        { path: "Custom Dictionary.txt", size: 16 },
        { path: "Preferences", size: 40 },
        { path: "obsidian.json", size: 200 },
      ],
    },
    {
      id: "vlc",
      name: "VLC media player",
      status: "not_found",
      folder: `${HOME}/Library/Preferences/org.videolan.vlc`,
      files: [],
    },
    {
      id: "vscode",
      name: "Visual Studio Code",
      status: "found",
      folder: `${SUPPORT}/Code/User`,
      files: [
        { path: "keybindings.json", size: 700 },
        { path: "settings.json", size: 160 },
        { path: "snippets/rust.json", size: 120 },
      ],
    },
  ],
};

export const nothingFound: { apps: ScannedApp[] } = {
  apps: scan.apps.map((app) => ({ ...app, status: "not_found", files: [] })),
};

/** The capture plan for every found app; `secret` flags keybindings.json. */
export function captureFiles(secret: boolean): { files: CaptureFile[] } {
  return {
    files: scan.apps.flatMap((app) =>
      app.files.map((file) => ({
        app_id: app.id,
        app_name: app.name,
        path: file.path,
        key: `${app.id}/${file.path}`,
        size: file.size,
        findings:
          secret && app.id === "vscode" && file.path === "keybindings.json"
            ? [{ line: 12, kind: "GitHub token", excerpt: "ghp_aB…6789" }]
            : [],
      })),
    ),
  };
}

const text = (lines: [RestoreLine, string][], hidden = 0): RestoreFile["diff"] => ({
  kind: "text",
  hidden,
  lines: lines.map(([kind, text]) => ({ kind, text })),
});
type RestoreLine = "hunk" | "context" | "added" | "removed";

function file(app_id: string, path: string, target: string | null, action: RestoreFile["action"], extra: Partial<RestoreFile> = {}): RestoreFile {
  return {
    app_id,
    path,
    target,
    action,
    skip_reason: null,
    changes: [],
    flags: [],
    diff: { kind: "nothing" },
    ...extra,
  };
}

/** A Windows bundle restored on a Mac, as the engine plans it. */
export function restorePreview(running: string[] = []): RestorePreview {
  return {
    bundle_name: "From Windows laptop.zip",
    created: "2026-11-16T18:20:00Z",
    source_os: "Windows",
    target_os: "macOS",
    cross_os: true,
    running,
    apps: [
      {
        id: "git",
        name: "Git",
        files: [
          file("git", ".config/git/ignore", `${HOME}/.config/git/ignore`, "create", {
            changes: ["line endings: Windows (CRLF) → macOS (LF)"],
            diff: text([
              ["hunk", "@@ -0,0 +1,2 @@"],
              ["added", "Thumbs.db"],
              ["added", "desktop.ini"],
            ]),
          }),
          file("git", ".gitconfig", `${HOME}/.gitconfig`, "update", {
            changes: ["1 path moved into your macOS home folder"],
            flags: [
              {
                line: 5,
                message:
                  "`autocrlf = true` sets how Git changes line endings, which Windows and macOS usually set differently (`true` on Windows, `input` or not set on macOS); check it suits macOS; left as it was",
              },
              {
                line: 6,
                message:
                  "`C:/Program Files/Notepad++/notepad++.exe` is outside your home folder, so there's no matching place on macOS; left as it was",
              },
            ],
            diff: text([
              ["hunk", "@@ -2,8 +2,8 @@"],
              ["context", "\tname = Sam Example"],
              ["context", "[core]"],
              ["removed", "\tautocrlf = input"],
              ["removed", "\teditor = code --wait"],
              ["added", "\tautocrlf = true"],
              ["added", "\teditor = 'C:/Program Files/Notepad++/notepad++.exe' -multiInst"],
              ["added", `\texcludesfile = ${HOME}/.gitignore_global`],
            ]),
          }),
        ],
      },
      {
        id: "obsidian",
        name: "Obsidian",
        files: [
          file("obsidian", "Custom Dictionary.txt", `${SUPPORT}/obsidian/Custom Dictionary.txt`, "unchanged"),
          file("obsidian", "obsidian.json", `${SUPPORT}/obsidian/obsidian.json`, "update", {
            changes: ["1 path moved into your macOS home folder"],
            flags: [
              {
                line: 1,
                message:
                  "`D:\\Vaults\\Work` is outside your home folder, so there's no matching place on macOS; left as it was",
              },
            ],
            diff: text([
              ["hunk", "@@ -1 +1 @@"],
              ["removed", '{"vaults":{"5a1b":{"path":"/Users/sam/Documents/Old"}}}'],
              ["added", '{"vaults":{"5a1b":{"path":"/Users/sam/Documents/Notes"},"8c9d":{"path":"D:\\\\Vaults\\\\Work"}}}'],
            ]),
          }),
        ],
      },
      {
        id: "vlc",
        name: "VLC media player",
        files: [
          file("vlc", "vlc-qt-interface.ini", null, "skip", { skip_reason: "only used on Windows" }),
          file("vlc", "vlcrc", `${HOME}/Library/Preferences/org.videolan.vlc/vlcrc`, "create", {
            changes: [
              "2 paths moved into your macOS home folder",
              "line endings: Windows (CRLF) → macOS (LF)",
            ],
            diff: text(
              [
                ["hunk", "@@ -0,0 +1,6 @@"],
                ["added", "[core]"],
                ["added", "volume-step=12.8"],
              ],
              40,
            ),
          }),
        ],
      },
      {
        id: "vscode",
        name: "Visual Studio Code",
        files: [
          file("vscode", "keybindings.json", `${SUPPORT}/Code/User/keybindings.json`, "update", {
            changes: ["3 shortcuts: Ctrl → Cmd", "line endings: Windows (CRLF) → macOS (LF)"],
            flags: [
              { line: 6, message: "`ctrl+tab` would become `cmd+tab`, which macOS keeps for itself; left as it was" },
              { line: 7, message: "`win+e` uses the Windows key, which macOS doesn't have; left as it was" },
              { line: 8, message: "a shortcut here depends on the OS (`isWindows`); check it does what you want on macOS" },
            ],
            diff: text([
              ["hunk", "@@ -3,4 +3,4 @@"],
              ["context", '    { "key": "cmd+k cmd+t", "command": "workbench.action.selectTheme" },'],
              ["removed", '    { "key": "cmd+w", "command": "workbench.action.closeActiveEditor" },'],
              ["added", '    { "key": "ctrl+tab", "command": "workbench.action.nextEditor" },'],
            ]),
          }),
          file("vscode", "settings.json", `${SUPPORT}/Code/User/settings.json`, "update", {
            changes: ["1 path moved into your macOS home folder"],
            flags: [
              {
                line: 4,
                message:
                  "`C:\\Program Files\\Git\\bin\\git.exe` is outside your home folder, so there's no matching place on macOS; left as it was",
              },
            ],
            diff: { kind: "line_endings_only" },
          }),
          file("vscode", "snippets/rust.json", `${SUPPORT}/Code/User/snippets/rust.json`, "unchanged"),
        ],
      },
    ],
  };
}

/** The same bundle when this computer already has every file. */
export function alreadyMatches(): RestorePreview {
  const preview = restorePreview();
  return {
    ...preview,
    apps: preview.apps.map((app) => ({
      ...app,
      files: app.files.map((f) => ({ ...f, action: "unchanged", changes: [], flags: [], diff: { kind: "nothing" } })),
    })),
  };
}

export function undoPreview(running: string[] = []): UndoPreview {
  return {
    id: "2026-11-17T20-10-00Z",
    bundle_name: "From Windows laptop.zip",
    created: "2026-11-17T20:10:00Z",
    running,
    steps: [
      { app_name: "Visual Studio Code", kind: "put_back", path: `${SUPPORT}/Code/User/settings.json` },
      { app_name: "Visual Studio Code", kind: "put_back", path: `${SUPPORT}/Code/User/keybindings.json` },
      { app_name: "Git", kind: "delete", path: `${HOME}/.config/git/ignore` },
    ],
  };
}
