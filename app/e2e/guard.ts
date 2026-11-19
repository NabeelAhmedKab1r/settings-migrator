// The safety guard every test setup in e2e/ runs first. These tests drive
// the real app and write settings files, so they run only in GitHub
// Actions, and only against a stand-in home folder in a temp folder:
// never a real home folder, and never on someone's own computer.

import { homedir, tmpdir } from "node:os";
import { isAbsolute, join, parse, relative, resolve, sep } from "node:path";

/** Folders whose direct children are people's home folders. */
const HOME_PARENTS = ["/Users", "/home", "/var/home", "C:\\Users", "D:\\Users"];

/**
 * Returns the stand-in home folder from SETTINGS_MIGRATOR_HOME, or throws
 * if anything about it could reach real settings.
 */
export function testHome(): string {
  if (process.env.GITHUB_ACTIONS !== "true") {
    fail("these tests run only in GitHub Actions, never on your own computer");
  }
  const raw = process.env.SETTINGS_MIGRATOR_HOME;
  if (!raw) fail("SETTINGS_MIGRATOR_HOME isn't set");
  if (!isAbsolute(raw)) fail(`SETTINGS_MIGRATOR_HOME must be an absolute path, not ${raw}`);
  const home = resolve(raw);
  checkNotRealHome(home);
  return home;
}

/** Throws if `path` is, or contains, a real home folder. */
export function checkNotRealHome(path: string): void {
  const target = norm(path);
  const realHomes = [homedir(), process.env.HOME, process.env.USERPROFILE]
    .filter((home): home is string => !!home)
    .map(norm);
  for (const home of realHomes) {
    if (target === home) fail(`${path} is a real home folder`);
    if (inside(home, target)) fail(`${path} contains the real home folder ${home}`);
  }
  for (const parent of HOME_PARENTS.map(norm)) {
    if (target === parent || inside(parent, target)) {
      fail(`${path} is, or holds, the folder of real home folders ${parent}`);
    }
    if (parse(target).dir === parent) fail(`${path} looks like someone's home folder`);
  }
  if (target === norm(parse(target).root)) fail(`${path} is the root of a drive`);
  const temps = [process.env.RUNNER_TEMP, tmpdir()].filter((dir): dir is string => !!dir).map(norm);
  if (!temps.some((temp) => inside(target, temp))) {
    fail(`${path} must be inside a temp folder (${temps.join(" or ")})`);
  }
}

/** Whether `child` is strictly inside `parent`. */
function inside(child: string, parent: string): boolean {
  const rel = relative(parent, child);
  return rel !== "" && !rel.startsWith("..") && !isAbsolute(rel);
}

/** Absolute, without a trailing separator, ignoring case (as Windows and macOS do by default). */
function norm(path: string): string {
  const full = resolve(path);
  const trimmed = full.length > parse(full).root.length && full.endsWith(sep) ? full.slice(0, -1) : full;
  return trimmed.toLowerCase();
}

function fail(why: string): never {
  throw new Error(`Refusing to run the tests: ${why}.`);
}

/** A folder for one test's stand-in home, inside the checked one. */
export function homeFor(name: string): string {
  const home = join(testHome(), name);
  checkNotRealHome(home);
  return home;
}
