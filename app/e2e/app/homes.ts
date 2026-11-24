// The stand-in home folders the real-app tests run against, one per spec,
// made fresh from the sample settings in the engine's test fixtures.

import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { checkNotRealHome, homeFor } from "../guard";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

/** The Windows sample home: settings for all 5 apps, no secrets. */
export const WINDOWS_SAMPLE = join(repo, "crates/engine/tests/fixtures/cross-os/windows-home");

/** Screenshots, uploaded by CI as an artifact. */
export const SCREENSHOTS = join(repo, "app/e2e/app/screenshots");

/** Logs from tauri-driver, the Edge driver and WebdriverIO, uploaded by CI as an artifact. */
export const LOGS = join(repo, "app/e2e/app/logs");

/** The built app, from `npm run tauri -- build --debug --no-bundle`. */
export const APP_BINARY = join(repo, "target/debug/settings-migrator-desktop.exe");

/** Not a real token: the right shape for the secret guard to flag. */
export const FAKE_TOKEN = "ghp_" + "aBcDeFgHiJkLmNoPqRsTuVwXyZ" + "0123456789";

export interface SpecPaths {
  /** The stand-in home folder the app uses. */
  home: string;
  /** Where the app saves its settings file, outside the home. */
  bundle: string;
}

/** Makes a fresh home for the spec called `name`, with what it needs. */
export function prepareHome(name: string): SpecPaths {
  const root = homeFor(name);
  rmSync(root, { recursive: true, force: true });
  const home = join(root, "home");
  const bundle = join(root, "out", "bundle.zip");
  checkNotRealHome(home);
  mkdirSync(dirname(bundle), { recursive: true });
  cpSync(WINDOWS_SAMPLE, home, { recursive: true });
  if (name === "save-restore-undo") {
    // Something for the secret review list to flag.
    const keys = join(home, "AppData/Roaming/Code/User/keybindings.json");
    writeFileSync(keys, `// token ${FAKE_TOKEN}\r\n[]\r\n`);
  }
  return { home, bundle };
}

/** The spec's paths, as `wdio.conf.ts` passed them to the test. */
export function specPaths(): SpecPaths {
  const home = process.env.E2E_HOME;
  const bundle = process.env.E2E_BUNDLE;
  if (!home || !bundle) throw new Error("run these through wdio.conf.ts");
  checkNotRealHome(home);
  return { home, bundle };
}
