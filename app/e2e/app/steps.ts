// Small helpers for driving the app's screens by what the user sees.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { unzipSync, strFromU8 } from "fflate";
import { SCREENSHOTS } from "./homes";

/** Clicks the button or label whose text contains `text`. */
export async function click(text: string) {
  const el = await $(`//*[(self::button or self::label)][contains(normalize-space(.), "${text}")]`);
  await el.waitForClickable();
  await el.click();
}

/** Waits for the screen titled `title`. */
export async function screen(title: string) {
  await $(`h1=${title}`).waitForDisplayed();
}

/** Waits until the innermost element holding `text` is on screen. */
export async function shows(text: string) {
  const has = `contains(normalize-space(.), "${text}")`;
  await $(`//body//*[${has} and not(*[${has}])]`).waitForDisplayed();
}

export async function shot(name: string) {
  await browser.saveScreenshot(join(SCREENSHOTS, `${process.env.E2E_SPEC}-${name}.png`));
}

/** The files in a settings bundle, by their path inside it. */
export function bundleFiles(path: string): Record<string, string> {
  const entries = unzipSync(readFileSync(path));
  return Object.fromEntries(Object.entries(entries).map(([name, bytes]) => [name, strFromU8(bytes)]));
}
