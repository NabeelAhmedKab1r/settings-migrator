// The safety guard refuses anything that could reach real settings.

import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { expect, test } from "@playwright/test";
import { testHome } from "../guard";

const saved = { ...process.env };

test.afterEach(() => {
  process.env = { ...saved };
});

function withHome(home: string | undefined) {
  if (home === undefined) delete process.env.SETTINGS_MIGRATOR_HOME;
  else process.env.SETTINGS_MIGRATOR_HOME = home;
  return () => testHome();
}

const temp = process.env.RUNNER_TEMP ?? tmpdir();

test("accepts a folder inside the runner's temp folder", () => {
  expect(withHome(join(temp, "some-test-home"))()).toBe(join(temp, "some-test-home"));
});

test("runs only in GitHub Actions", () => {
  delete process.env.GITHUB_ACTIONS;
  expect(withHome(join(temp, "some-test-home"))).toThrow(/only in GitHub Actions/);
});

test("needs SETTINGS_MIGRATOR_HOME", () => {
  expect(withHome(undefined)).toThrow(/isn't set/);
  expect(withHome("")).toThrow(/isn't set/);
  expect(withHome("relative/home")).toThrow(/absolute path/);
});

test("refuses the real home folder and anything holding it", () => {
  expect(withHome(homedir())).toThrow(/real home folder/);
  expect(withHome(homedir().toUpperCase())).toThrow(/real home folder/);
  expect(withHome(dirname(homedir()))).toThrow(/Refusing/);
});

test("refuses folders outside a temp folder", () => {
  expect(withHome(join(homedir(), "Documents", "fake-home"))).toThrow(/inside a temp folder/);
});
