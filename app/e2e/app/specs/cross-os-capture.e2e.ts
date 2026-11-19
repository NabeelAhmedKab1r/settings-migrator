// Saves the Windows sample settings with the real app. CI then restores
// this file with the command-line tool on the macOS runner and checks
// every file (the cross-os-app-restore job).

import { existsSync } from "node:fs";
import { specPaths } from "../homes";
import { bundleFiles, click, screen, shot, shows } from "../steps";

const { bundle } = specPaths();

describe("saving on Windows for a Mac", () => {
  it("saves all 5 apps' settings", async () => {
    await screen("Move your app settings");
    await click("Save settings from this computer");
    await shows("Found settings for 5 apps");
    await click("Next: choose apps");
    await click("Next: check and save");
    await shows("13 settings files from 5 apps");
    await click("Save 13 files");
    await screen("Your settings are saved");
    await shot("saved");

    expect(existsSync(bundle)).toBe(true);
    const manifest = JSON.parse(bundleFiles(bundle)["manifest.json"]);
    expect(manifest.source_os).toBe("windows");
    expect(manifest.apps.map((app: { id: string }) => app.id).sort()).toEqual([
      "git",
      "obs",
      "obsidian",
      "vlc",
      "vscode",
    ]);
  });
});
