// The engine's commands (app/src-tauri/src/commands.rs) and the data they
// return. Every command that writes has a preview command before it.

import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  os: string;
  /** Set when trying the app out on a stand-in home folder. */
  test_home: string | null;
}

export interface ScannedApp {
  id: string;
  name: string;
  status: "found" | "not_found" | "unsupported";
  folder: string | null;
  files: { path: string; size: number }[];
}

export interface SecretFinding {
  line: number;
  kind: string;
  /** Shortened so the secret itself is never shown. */
  excerpt: string;
}

export interface CaptureFile {
  app_id: string;
  app_name: string;
  path: string;
  /** `app/path`: how a flagged file is skipped or allowed. */
  key: string;
  size: number;
  findings: SecretFinding[];
}

export interface Flag {
  /** 0 means the whole file. */
  line: number;
  message: string;
}

export type Diff =
  | { kind: "nothing" }
  | { kind: "line_endings_only" }
  | { kind: "binary"; old: number | null; new: number }
  | {
      kind: "text";
      lines: { kind: "hunk" | "context" | "added" | "removed"; text: string }[];
      hidden: number;
    };

export interface RestoreFile {
  app_id: string;
  path: string;
  target: string | null;
  action: "create" | "update" | "unchanged" | "skip";
  skip_reason: string | null;
  changes: string[];
  flags: Flag[];
  diff: Diff;
}

export interface RestorePreview {
  bundle_name: string;
  created: string;
  source_os: string;
  target_os: string;
  cross_os: boolean;
  apps: { id: string; name: string; files: RestoreFile[] }[];
  running: string[];
}

export interface UndoPreview {
  id: string;
  bundle_name: string;
  created: string;
  steps: { app_name: string; kind: "put_back" | "delete"; path: string }[];
  running: string[];
}

export const appInfo = () => invoke<AppInfo>("app_info");

export const scanApps = () =>
  invoke<{ apps: ScannedApp[] }>("scan_apps").then((result) => result.apps);

export const planCapture = (apps: string[]) =>
  invoke<{ files: CaptureFile[] }>("plan_capture", { apps }).then(
    (result) => result.files,
  );

export const saveBundle = (
  apps: string[],
  skip: string[],
  allow: string[],
  path: string,
) =>
  invoke<{ path: string; files: number }>("save_bundle", {
    apps,
    skip,
    allow,
    path,
  });

export const previewRestore = (bundle: string) =>
  invoke<RestorePreview>("preview_restore", { bundle });

export const restore = (bundle: string, apps: string[]) =>
  invoke<{ files: number }>("restore", { bundle, apps });

export const previewUndo = () => invoke<UndoPreview | null>("preview_undo");

export const undo = (id: string) => invoke<void>("undo", { id });

/** Command errors arrive as plain text. */
export function errorText(err: unknown): string {
  return typeof err === "string" ? err : String(err);
}
