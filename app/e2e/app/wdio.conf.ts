// WebdriverIO drives the real desktop app through tauri-driver, which only
// supports Windows (and Linux) of the OSes we build for, so this runs on
// the Windows runner only. See the e2e-windows job in .github/workflows/ci.yml.
//
// Built to fail within a few minutes, not hang: tauri-driver must be
// listening within 30 s, a session must start within 60 s with no retries,
// and everything tauri-driver and WebdriverIO print goes to e2e/app/logs/
// (uploaded by CI) so a failure says why.

import { spawn, type ChildProcess } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, openSync } from "node:fs";
import { connect } from "node:net";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { testHome } from "../guard";
import { APP_BINARY, LOGS, SCREENSHOTS, prepareHome } from "./homes";

// Fails before anything starts if the stand-in home isn't safe.
testHome();

const HOST = "127.0.0.1";
const PORT = 4444;

let driver: ChildProcess | undefined;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/*.e2e.ts"],
  // One app at a time: each spec gets its own app and home.
  maxInstances: 1,
  hostname: HOST,
  port: PORT,
  capabilities: [
    {
      // @ts-expect-error tauri-driver's own capability
      "tauri:options": { application: APP_BINARY },
    },
  ],
  logLevel: "info",
  // WebdriverIO's own logs, one file per worker.
  outputDir: LOGS,
  // The defaults (120 s, 3 retries) wait 8 minutes for a session that
  // isn't coming. A debug build of the app starts well within 60 s.
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 0,
  specFileRetries: 0,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { ui: "bdd", timeout: 60_000 },
  waitforTimeout: 20_000,

  onPrepare() {
    testHome();
    if (!existsSync(APP_BINARY)) {
      throw new Error(`Build the app first (npm run tauri -- build --debug --no-bundle): ${APP_BINARY} is missing`);
    }
    mkdirSync(SCREENSHOTS, { recursive: true });
    mkdirSync(LOGS, { recursive: true });
  },

  // Runs in the spec's worker before the app starts: makes the spec's home
  // and starts tauri-driver with it, so the app sees only that folder.
  async beforeSession(_config, _capabilities, specs) {
    const spec = specs[0].startsWith("file:") ? fileURLToPath(specs[0]) : specs[0];
    const name = basename(spec).replace(/\.e2e\.ts$/, "");
    const { home, bundle } = prepareHome(name);
    process.env.E2E_HOME = home;
    process.env.E2E_BUNDLE = bundle;
    process.env.E2E_SPEC = name;

    // tauri-driver's output, and through it the Edge driver's and the app's.
    mkdirSync(LOGS, { recursive: true });
    const logPath = join(LOGS, `${name}-tauri-driver.log`);
    const nativeDriver = process.env.EDGE_DRIVER;
    const args = nativeDriver ? ["--native-driver", nativeDriver] : [];
    const command = process.env.TAURI_DRIVER ?? "tauri-driver";
    appendFileSync(logPath, `> ${command} ${args.join(" ")}\n> app: ${APP_BINARY}\n> home: ${home}\n\n`);
    const log = openSync(logPath, "a");
    driver = spawn(command, args, {
      stdio: ["ignore", log, log],
      env: {
        ...process.env,
        RUST_BACKTRACE: "1",
        SETTINGS_MIGRATOR_HOME: home,
        SETTINGS_MIGRATOR_TEST_SAVE_AS: bundle,
        SETTINGS_MIGRATOR_TEST_OPEN: bundle,
      },
    });
    await waitForDriver(driver, logPath);
  },

  async afterTest(test, _context, { passed }) {
    if (!passed) {
      const name = `${process.env.E2E_SPEC}-FAILED-${test.title.replace(/\W+/g, "-")}.png`;
      await browser.saveScreenshot(join(SCREENSHOTS, name)).catch(() => {});
    }
  },

  afterSession() {
    driver?.kill();
    driver = undefined;
  },
};

/** Waits until tauri-driver accepts connections; throws if it exits or takes over 30 s. */
async function waitForDriver(child: ChildProcess, logPath: string) {
  let exited: string | undefined;
  child.once("exit", (code, signal) => (exited = `exited with ${signal ?? `code ${code}`}`));
  child.once("error", (err) => (exited = `couldn't start: ${err.message}`));
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (exited) throw new Error(`tauri-driver ${exited}. See ${logPath}`);
    if (await listening()) return;
    await new Promise((done) => setTimeout(done, 250));
  }
  child.kill();
  throw new Error(`tauri-driver wasn't listening on ${HOST}:${PORT} after 30 s. See ${logPath}`);
}

function listening(): Promise<boolean> {
  return new Promise((done) => {
    const socket = connect(PORT, HOST);
    socket.once("connect", () => {
      socket.destroy();
      done(true);
    });
    socket.once("error", () => done(false));
  });
}
