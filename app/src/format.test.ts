import { describe, expect, it } from "vitest";
import { bundleName, count, list, quoted, size } from "./format";

describe("format", () => {
  it("counts in plain words", () => {
    expect(count(1, "file")).toBe("1 file");
    expect(count(3, "file")).toBe("3 files");
    expect(count(2, "match", "matches")).toBe("2 matches");
  });

  it("shows sizes", () => {
    expect(size(512)).toBe("512 B");
    expect(size(2048)).toBe("2.0 KB");
    expect(size(3 * 1024 * 1024)).toBe("3.0 MB");
  });

  it("joins names", () => {
    expect(list([])).toBe("");
    expect(list(["VS Code"])).toBe("VS Code");
    expect(list(["VS Code", "OBS", "VLC"])).toBe("VS Code, OBS and VLC");
  });

  it("finds the quoted parts of engine messages", () => {
    expect(quoted("`ctrl+tab` would become `cmd+tab`")).toEqual([
      "",
      "ctrl+tab",
      " would become ",
      "cmd+tab",
      "",
    ]);
  });

  it("names new settings files by date", () => {
    expect(bundleName(new Date(2026, 0, 5))).toBe("Settings 2026-01-05.zip");
  });
});
