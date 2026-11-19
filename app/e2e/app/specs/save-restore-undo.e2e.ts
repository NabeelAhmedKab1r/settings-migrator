// The whole journey in the real app, against a stand-in Windows home:
// save (with a flagged file left out), restore preview, a change after the
// preview, restore, undo. Checks the files on disk after every step.

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { FAKE_TOKEN, specPaths } from "../homes";
import { bundleFiles, click, screen, shot, shows } from "../steps";

const { home, bundle } = specPaths();
const settings = join(home, "AppData/Roaming/Code/User/settings.json");
const gitconfig = join(home, ".gitconfig");
const read = (path: string) => readFileSync(path, "utf8");

describe("save, restore and undo in the real app", () => {
  const originalSettings = read(settings);
  const originalGitconfig = read(gitconfig);
  const editedSettings = '{\r\n    "editor.fontSize": 22\r\n}\r\n';

  it("finds the apps on this computer", async () => {
    await screen("Move your app settings");
    await shot("01-home");
    await click("Save settings from this computer");
    await shows("Found settings for 5 apps");
    await shot("02-scan");
    await click("Next: choose apps");
    await screen("Which apps should come with you?");
    await shot("03-choose");
  });

  it("flags the file with a token and leaves it out by default", async () => {
    await click("Next: check and save");
    await shows("1 file might hold a password or key");
    await shows("keybindings.json");
    // The excerpt is shortened, so the token itself never shows.
    expect(await $("body").getText()).not.toContain(FAKE_TOKEN);
    await shot("04-secret-review");
    expect(existsSync(bundle)).toBe(false);
  });

  it("saves the settings file without the flagged file", async () => {
    await click("Save 12 files");
    await screen("Your settings are saved");
    await shot("05-saved");
    const files = bundleFiles(bundle);
    expect(Object.keys(files)).toContain("manifest.json");
    expect(files["vscode/settings.json"]).toBe(originalSettings);
    expect(files["git/.gitconfig"]).toBe(originalGitconfig);
    expect(Object.keys(files)).not.toContain("vscode/keybindings.json");
    for (const text of Object.values(files)) expect(text).not.toContain(FAKE_TOKEN);
    await click("Done");
  });

  it("previews the restore without writing anything", async () => {
    // Change things after saving, so restoring has work to do.
    writeFileSync(settings, editedSettings);
    rmSync(gitconfig);

    await click("Put settings on this computer");
    await click("Choose settings file");
    await screen("Check the changes");
    await shows("Replaces yours");
    await shows("New");
    await shot("06-restore-preview");
    expect(read(settings)).toBe(editedSettings);
    expect(existsSync(gitconfig)).toBe(false);
  });

  it("refuses to restore when a file changed since the preview", async () => {
    const changedAgain = '{\r\n    "editor.fontSize": 30\r\n}\r\n';
    writeFileSync(settings, changedAgain);
    await click("Restore 2 files");
    await shows("Something changed since the preview was made");
    await shot("07-changed-since-preview");
    expect(read(settings)).toBe(changedAgain);
    expect(existsSync(gitconfig)).toBe(false);
    writeFileSync(settings, editedSettings);
  });

  it("restores after a fresh preview", async () => {
    // The screen reloaded the preview after refusing; load it once more
    // for the edit just made, then restore.
    await click("Back");
    await click("Choose settings file");
    await screen("Check the changes");
    await click("Restore 2 files");
    await screen("Your settings are in place");
    await shot("08-restored");
    expect(read(settings)).toBe(originalSettings);
    expect(read(gitconfig)).toBe(originalGitconfig);
  });

  it("undoes the restore", async () => {
    await click("Undo this");
    await screen("Undo the last restore?");
    await shows("Your earlier version comes back (1 file)");
    await shows("Removed, since they weren't here before (1 file)");
    await shot("09-undo-preview");
    expect(read(settings)).toBe(originalSettings);

    await click("Undo restore");
    await screen("Undone");
    await shot("10-undone");
    expect(read(settings)).toBe(editedSettings);
    expect(existsSync(gitconfig)).toBe(false);
  });

  it("has nothing left to undo", async () => {
    await click("Done");
    await click("Undo the last time");
    await shows("There's nothing to undo");
    await shot("11-nothing-to-undo");
  });
});
