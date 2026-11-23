// Every screen and error state of the desktop app, with a screenshot of
// each saved to e2e/ui/screenshots/<browser>/ (uploaded by CI).

import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { calls, happy, openApp, type Script } from "./engine";
import * as data from "./fixtures";

const SCREENSHOTS = join(import.meta.dirname, "screenshots");

async function shot(page: Page, name: string) {
  await page.screenshot({
    path: join(SCREENSHOTS, test.info().project.name, `${name}.png`),
    fullPage: true,
  });
}

const button = (page: Page, name: string | RegExp) => page.getByRole("button", { name });
const heading = (page: Page, name: string) => page.getByRole("heading", { level: 1, name });

function script(changes: Partial<Script>): Script {
  return { ...happy(), ...changes } as Script;
}

/** From the home screen to the "Check and save" screen. */
async function toCapture(page: Page) {
  await button(page, /Save settings from this computer/).click();
  await button(page, "Next: choose apps").click();
  await button(page, "Next: check and save").click();
  await expect(heading(page, "Check and save")).toBeVisible();
}

/** From the home screen to the restore preview. */
async function toPreview(page: Page) {
  await button(page, /Put settings on this computer/).click();
  await button(page, /Choose settings file/).click();
}

test.describe("home", () => {
  test("offers saving, restoring and undo", async ({ page }) => {
    await openApp(page, happy());
    await expect(heading(page, "Move your app settings")).toBeVisible();
    await expect(page.getByText("This computer runs macOS.")).toBeVisible();
    await expect(button(page, /Undo the last time/)).toBeVisible();
    await shot(page, "home");
  });

  test("says when it's using a stand-in home folder", async ({ page }) => {
    await openApp(page, script({ app_info: [{ ok: { ...data.appInfo, test_home: "/tmp/fake-home" } }] }));
    await expect(page.getByText("Test mode: using /tmp/fake-home")).toBeVisible();
    await shot(page, "home-test-mode");
  });
});

test.describe("scan and choose apps", () => {
  test("lists found and missing apps", async ({ page }) => {
    await openApp(page, happy());
    await button(page, /Save settings from this computer/).click();
    await expect(page.getByText("Found settings for 4 apps of the 5")).toBeVisible();
    await expect(page.getByText("Not on this computer")).toBeVisible();
    await shot(page, "scan");
  });

  test("can't go on when no apps are found", async ({ page }) => {
    await openApp(page, script({ scan_apps: [{ ok: data.nothingFound }] }));
    await button(page, /Save settings from this computer/).click();
    await expect(page.getByText(/None of the apps we know/)).toBeVisible();
    await expect(button(page, "Next: choose apps")).toBeDisabled();
    await shot(page, "scan-nothing-found");
  });

  test("shows a scan error and can look again", async ({ page }) => {
    await openApp(page, script({ scan_apps: [{ err: "C:\\Users\\Sam: Access is denied." }, { ok: data.scan }] }));
    await button(page, /Save settings from this computer/).click();
    await expect(page.getByRole("alert")).toContainText("Couldn't look for apps");
    await expect(page.getByRole("alert")).toContainText("Access is denied");
    await shot(page, "scan-error");
  });

  test("chooses apps, all ticked to start with", async ({ page }) => {
    await openApp(page, happy());
    await button(page, /Save settings from this computer/).click();
    await button(page, "Next: choose apps").click();
    await expect(heading(page, "Which apps should come with you?")).toBeVisible();
    await expect(page.getByRole("checkbox")).toHaveCount(4);
    for (const box of await page.getByRole("checkbox").all()) await expect(box).toBeChecked();
    await button(page, "3 files").first().click();
    await shot(page, "choose-apps");

    await button(page, "Choose none").click();
    await expect(button(page, "Next: check and save")).toBeDisabled();
    await shot(page, "choose-apps-none");
  });
});

test.describe("capture", () => {
  test("saves when nothing is flagged", async ({ page }) => {
    await openApp(page, happy());
    await toCapture(page);
    await expect(page.getByText("12 settings files from 4 apps")).toBeVisible();
    await shot(page, "capture");
    await button(page, "Save 12 files…").click();
    await expect(heading(page, "Your settings are saved")).toBeVisible();
    await shot(page, "capture-saved");
    const [saved] = await calls(page, "save_bundle");
    expect(saved).toMatchObject({ skip: [], allow: [], path: "/Users/sam/Desktop/Settings 2026-11-17.zip" });
  });

  test("lists a possible secret and leaves the file out by default", async ({ page }) => {
    await openApp(page, script({ plan_capture: [{ ok: data.captureFiles(true) }] }));
    await toCapture(page);
    const review = page.getByRole("region", { name: /might hold a password or key/ });
    await expect(review).toContainText("keybindings.json");
    await expect(review).toContainText("Line 12: GitHub token");
    await expect(page.getByLabel(/Leave this file out/)).toBeChecked();
    await expect(page.getByText("2 files, 1 left out")).toBeVisible();
    await shot(page, "capture-secret-flagged");
    await button(page, "Save 11 files…").click();
    await expect(heading(page, "Your settings are saved")).toBeVisible();
    const [saved] = await calls(page, "save_bundle");
    expect(saved).toMatchObject({ skip: ["vscode/keybindings.json"], allow: [] });
  });

  test("includes a flagged file only when the user says it's safe", async ({ page }) => {
    await openApp(page, script({ plan_capture: [{ ok: data.captureFiles(true) }] }));
    await toCapture(page);
    await page.getByLabel(/Include it/).check();
    await expect(button(page, "Save 12 files…")).toBeEnabled();
    await shot(page, "capture-secret-allowed");
    await button(page, "Save 12 files…").click();
    await expect(heading(page, "Your settings are saved")).toBeVisible();
    const [saved] = await calls(page, "save_bundle");
    expect(saved).toMatchObject({ skip: [], allow: ["vscode/keybindings.json"] });
  });

  test("never replaces an existing file", async ({ page }) => {
    const exists =
      "There's already a file called Settings 2026-11-17.zip. Choose another name, so it isn't replaced.";
    await openApp(page, script({ save_bundle: [{ err: exists }] }));
    await toCapture(page);
    await button(page, "Save 12 files…").click();
    await expect(page.getByRole("alert")).toContainText("Not saved");
    await expect(page.getByRole("alert")).toContainText("already a file");
    await expect(heading(page, "Check and save")).toBeVisible();
    await shot(page, "capture-file-exists");
  });

  test("does nothing when the save dialog is cancelled", async ({ page }) => {
    await openApp(page, script({ "plugin:dialog|save": [{ ok: null }] }));
    await toCapture(page);
    await button(page, "Save 12 files…").click();
    // Wait until the dialog was asked for and answered, so the check below
    // can't pass just because the click hadn't been handled yet.
    await expect.poll(() => calls(page, "plugin:dialog|save")).toHaveLength(1);
    await expect(button(page, "Save 12 files…")).toBeEnabled();
    await expect(heading(page, "Check and save")).toBeVisible();
    expect(await calls(page, "save_bundle")).toHaveLength(0);
  });

  test("shows an error reading the settings", async ({ page }) => {
    await openApp(page, script({ plan_capture: [{ err: "settings.json: Permission denied" }] }));
    await toCapture(page);
    await expect(page.getByRole("alert")).toContainText("Permission denied");
    await shot(page, "capture-error");
  });
});

test.describe("restore preview", () => {
  test("asks for the settings file", async ({ page }) => {
    await openApp(page, happy());
    await button(page, /Put settings on this computer/).click();
    await expect(heading(page, "Put settings on this computer")).toBeVisible();
    await shot(page, "restore-choose-file");
  });

  test("explains a file that isn't a settings bundle", async ({ page }) => {
    await openApp(page, script({
      preview_restore: [{ err: "This file can't be read as a settings bundle. (invalid Zip archive)" }],
    }));
    await toPreview(page);
    await expect(page.getByRole("alert")).toContainText("Couldn't open that file");
    await expect(page.getByRole("alert")).toContainText("can't be read as a settings bundle");
    await shot(page, "restore-bad-file");
  });

  test("shows every change, what was adjusted and what couldn't be", async ({ page }) => {
    await openApp(page, happy());
    await toPreview(page);
    await expect(heading(page, "Check the changes")).toBeVisible();
    await expect(page.getByText("Moving from Windows to macOS")).toBeVisible();
    await expect(page.getByText("7 things couldn't be adjusted")).toBeVisible();
    await expect(page.getByText("Adjusted: 3 shortcuts: Ctrl → Cmd")).toBeVisible();
    await expect(page.getByRole("list", { name: "Couldn't be adjusted" })).toHaveCount(4);
    await expect(page.getByText("Not put back: only used on Windows.")).toBeVisible();
    await expect(page.getByText("Only the line endings change.")).toBeVisible();
    await button(page, "Show changes").first().click();
    await expect(page.getByLabel("Changes to this file")).toContainText("Thumbs.db");
    await expect(button(page, "Restore 6 files")).toBeEnabled();
    await shot(page, "restore-preview");
  });

  test("restores only the apps left ticked", async ({ page }) => {
    await openApp(page, happy());
    await toPreview(page);
    await page.getByRole("checkbox", { name: /VLC media player/ }).uncheck();
    await expect(button(page, "Restore 5 files")).toBeEnabled();
    await shot(page, "restore-preview-one-app-unticked");
    await button(page, "Restore 5 files").click();
    await expect(heading(page, "Your settings are in place")).toBeVisible();
    const [restored] = await calls(page, "restore");
    expect(restored.apps).toEqual(["git", "obsidian", "vscode"]);
  });

  test("waits while an app it changes is open", async ({ page }) => {
    await openApp(page, script({
      preview_restore: [{ ok: data.restorePreview(["Visual Studio Code"]) }, { ok: data.restorePreview() }],
    }));
    await toPreview(page);
    await expect(page.getByText("Please quit Visual Studio Code first")).toBeVisible();
    await expect(button(page, "Restore 6 files")).toBeDisabled();
    await shot(page, "restore-app-open");
    await button(page, "I've quit, check again").click();
    await expect(page.getByText("Please quit Visual Studio Code first")).toBeHidden();
    await expect(button(page, "Restore 6 files")).toBeEnabled();
  });

  test("refuses when the files changed since the preview, and shows the new one", async ({ page }) => {
    const changed =
      "Something changed since the preview was made. Look over the new preview, then restore again.";
    await openApp(page, script({
      restore: [{ err: changed }, { ok: { files: 6 } }],
      preview_restore: [{ ok: data.restorePreview() }, { ok: data.restorePreview() }],
    }));
    await toPreview(page);
    await button(page, "Restore 6 files").click();
    await expect(page.getByRole("alert")).toContainText("Nothing was restored");
    await expect(page.getByRole("alert")).toContainText("Something changed since the preview");
    await expect(heading(page, "Check the changes")).toBeVisible();
    await expect.poll(() => calls(page, "preview_restore")).toHaveLength(2);
    await shot(page, "restore-changed-since-preview");
    await button(page, "Restore 6 files").click();
    await expect(heading(page, "Your settings are in place")).toBeVisible();
  });

  test("has nothing to do when everything already matches", async ({ page }) => {
    await openApp(page, script({ preview_restore: [{ ok: data.alreadyMatches() }] }));
    await toPreview(page);
    await expect(page.getByText("Everything already matches")).toBeVisible();
    await expect(button(page, "Restore 0 files")).toBeDisabled();
    await shot(page, "restore-already-matches");
  });

  test("says what to check after restoring, and offers undo", async ({ page }) => {
    await openApp(page, happy());
    await toPreview(page);
    await button(page, "Restore 6 files").click();
    await expect(heading(page, "Your settings are in place")).toBeVisible();
    await expect(page.getByText("7 things couldn't be adjusted")).toBeVisible();
    await shot(page, "restore-done");
    await button(page, "Undo this").click();
    await expect(heading(page, "Undo the last restore?")).toBeVisible();
  });
});

test.describe("undo", () => {
  test("shows what comes back and what is removed", async ({ page }) => {
    await openApp(page, happy());
    await button(page, /Undo the last time/).click();
    await expect(page.getByText("Your earlier version comes back (2 files)")).toBeVisible();
    await expect(page.getByText("Removed, since they weren't here before (1 file)")).toBeVisible();
    await shot(page, "undo-preview");
    await button(page, "Undo restore").click();
    await expect(heading(page, "Undone")).toBeVisible();
    expect(await calls(page, "undo")).toEqual([{ id: "2026-11-17T20-10-00Z" }]);
    await shot(page, "undo-done");
  });

  test("waits while an app it changes is open", async ({ page }) => {
    await openApp(page, script({
      preview_undo: [{ ok: data.undoPreview(["Visual Studio Code"]) }, { ok: data.undoPreview() }],
    }));
    await button(page, /Undo the last time/).click();
    await expect(page.getByText("Please quit Visual Studio Code first")).toBeVisible();
    await expect(button(page, "Undo restore")).toBeDisabled();
    await shot(page, "undo-app-open");
    await button(page, "I've quit, check again").click();
    await expect(button(page, "Undo restore")).toBeEnabled();
  });

  test("has nothing to undo", async ({ page }) => {
    await openApp(page, script({ preview_undo: [{ ok: null }] }));
    await button(page, /Undo the last time/).click();
    await expect(page.getByText("There's nothing to undo")).toBeVisible();
    await shot(page, "undo-nothing");
  });

  test("shows an undo error and what's true now", async ({ page }) => {
    await openApp(page, script({
      undo: [{ err: "Something changed since this was shown. Go back and look again." }],
    }));
    await button(page, /Undo the last time/).click();
    await button(page, "Undo restore").click();
    await expect(page.getByRole("alert")).toContainText("Nothing was undone");
    await shot(page, "undo-error");
  });
});
