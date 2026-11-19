// WebdriverIO drives the real desktop app through tauri-driver, which only
// supports Windows (and Linux) of the OSes we build for, so this runs on
// the Windows runner only. See the e2e-windows job in .github/workflows/ci.yml.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { testHome } from "../guard";
import { APP_BINARY, SCREENSHOTS, prepareHome } from "./homes";

// Fails before anything starts if the stand-in home isn't safe.
testHome();

let driver: ChildProcess | undefined;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/*.e2e.ts"],
  // One app at a time: each spec gets its own app and home.
  maxInstances: 1,
  hostname: "127.0.0.1",
  port: 4444,
  capabilities: [
    {
      // @ts-expect-error tauri-driver's own capability
      "tauri:options": { application: APP_BINARY },
    },
  ],
  logLevel: "warn",
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 120_000 },
  waitforTimeout: 20_000,

  onPrepare() {
    testHome();
    if (!existsSync(APP_BINARY)) {
      throw new Error(`Build the app first (npm run tauri -- build --debug --no-bundle): ${APP_BINARY} is missing`);
    }
    mkdirSync(SCREENSHOTS, { recursive: true });
  },

  // Runs in the spec's worker before the app starts: makes the spec's home
  // and starts tauri-driver with it, so the app sees only that folder.
  beforeSession(_config, _capabilities, specs) {
    const spec = specs[0].startsWith("file:") ? fileURLToPath(specs[0]) : specs[0];
    const name = basename(spec).replace(/\.e2e\.ts$/, "");
    const { home, bundle } = prepareHome(name);
    process.env.E2E_HOME = home;
    process.env.E2E_BUNDLE = bundle;
    process.env.E2E_SPEC = name;
    const nativeDriver = process.env.EDGE_DRIVER;
    driver = spawn(process.env.TAURI_DRIVER ?? "tauri-driver", nativeDriver ? ["--native-driver", nativeDriver] : [], {
      stdio: [null, process.stdout, process.stderr],
      env: {
        ...process.env,
        SETTINGS_MIGRATOR_HOME: home,
        SETTINGS_MIGRATOR_TEST_SAVE_AS: bundle,
        SETTINGS_MIGRATOR_TEST_OPEN: bundle,
      },
    });
    // Give tauri-driver a moment to listen.
    return new Promise((done) => setTimeout(done, 2_000));
  },

  async afterTest(test, _context, { passed }) {
    if (!passed) {
      const name = `${process.env.E2E_SPEC}-FAILED-${test.title.replace(/\W+/g, "-")}.png`;
      await browser.saveScreenshot(join(SCREENSHOTS, name));
    }
  },

  afterSession() {
    driver?.kill();
  },
};
