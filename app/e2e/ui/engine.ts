// Stands in for the engine in the browser: the app's invoke() calls go to
// window.__TAURI_INTERNALS__, which this replaces with scripted answers.
// Each command gets a list of answers, used in turn (the last one repeats).

import type { Page } from "@playwright/test";
import * as data from "./fixtures";

export type Answer = { ok: unknown } | { err: string };
export type Script = Record<string, Answer[]>;

/** Answers for a computer where everything works. */
export function happy(): Script {
  return {
    app_info: [{ ok: data.appInfo }],
    scan_apps: [{ ok: data.scan }],
    plan_capture: [{ ok: data.captureFiles(false) }],
    save_bundle: [{ ok: { path: "/Users/sam/Desktop/Settings 2026-11-17.zip", files: 12 } }],
    "plugin:dialog|save": [{ ok: "/Users/sam/Desktop/Settings 2026-11-17.zip" }],
    "plugin:dialog|open": [{ ok: "/Users/sam/Downloads/From Windows laptop.zip" }],
    preview_restore: [{ ok: data.restorePreview() }],
    restore: [{ ok: { files: 6 } }],
    preview_undo: [{ ok: data.undoPreview() }],
    undo: [{ ok: null }],
  };
}

/** Loads the app with `script` standing in for the engine. */
export async function openApp(page: Page, script: Script) {
  await page.addInitScript((script: Script) => {
    const calls: { cmd: string; args: unknown }[] = [];
    (window as unknown as { __calls: typeof calls }).__calls = calls;
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      async invoke(cmd: string, args: unknown) {
        calls.push({ cmd, args });
        const answers = script[cmd];
        if (!answers) throw `no scripted answer for ${cmd}`;
        const answer = answers.length > 1 ? answers.shift()! : answers[0];
        await new Promise((done) => setTimeout(done, 20));
        if ("err" in answer) throw answer.err;
        return answer.ok;
      },
      transformCallback: () => 0,
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    };
  }, script);
  await page.goto("/");
}

/** The commands the app sent, with their arguments. */
export async function calls(page: Page, cmd: string): Promise<Record<string, unknown>[]> {
  const all = await page.evaluate(() => (window as unknown as { __calls: { cmd: string; args: unknown }[] }).__calls);
  return all.filter((call) => call.cmd === cmd).map((call) => call.args as Record<string, unknown>);
}
