import { describe, expect, it } from "vitest";
import { FileTab } from "../types/tab";
import { createTabsFromPaths } from "./workspace";

function existing(path: string): FileTab {
  return {
    id: `id-${path}`,
    name: path.split(/[\\/]/).pop() || path,
    path,
    ext: "apk",
    status: "success",
    envelope: null,
    localError: null,
    createdAt: 1
  };
}

describe("createTabsFromPaths", () => {
  it("creates apk jobs in order and keeps aab as placeholder", () => {
    const result = createTabsFromPaths(
      ["D:/a.apk", "D:/b.aab", "D:/c.apk"],
      [],
      10,
      123
    );
    expect(result.createdTabs.map((item) => item.name)).toEqual(["a.apk", "b.aab", "c.apk"]);
    expect(result.createdTabs.map((item) => item.status)).toEqual(["pending", "placeholder", "pending"]);
    expect(result.jobs.map((item) => item.path)).toEqual(["D:/a.apk", "D:/c.apk"]);
  });

  it("deduplicates by path (case-insensitive) and counts unsupported", () => {
    const result = createTabsFromPaths(
      ["D:/A.apk", "D:/a.apk", "D:/x.txt", "D:/y.AAB"],
      [],
      10,
      123
    );
    expect(result.summary.duplicateCount).toBe(1);
    expect(result.summary.unsupportedCount).toBe(1);
    expect(result.createdTabs.map((item) => item.path)).toEqual(["D:/A.apk", "D:/y.AAB"]);
  });

  it("enforces max tabs without letting unsupported files consume slots", () => {
    const seed = Array.from({ length: 9 }, (_, index) => existing(`D:/seed-${index}.apk`));
    const result = createTabsFromPaths(
      ["D:/bad.txt", "D:/ok.apk", "D:/overflow.aab"],
      seed,
      10,
      123
    );
    expect(result.summary.unsupportedCount).toBe(1);
    expect(result.summary.droppedByLimit).toBe(1);
    expect(result.createdTabs.map((item) => item.path)).toEqual(["D:/ok.apk"]);
  });
});
