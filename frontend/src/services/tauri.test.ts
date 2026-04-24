import { describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(async (cmd: string) => {
    if (cmd === "pick_files") {
      return ["D:/tmp/a.apk", "D:/tmp/b.aab", "", "   "] as unknown;
    }
    return {
      success: true,
      data: {
        packageName: "com.example.app",
        appName: "Example",
        iconUrl: "",
        minSdkVersion: 21,
        targetSdkVersion: 34,
        compileSdkVersion: null,
        versionCode: 1,
        versionName: null,
        permissions: [],
        signers: [],
        abis: [],
        channel: "unknown"
      },
      errorCode: null,
      errorMessage: null,
      warnings: []
    } as unknown;
  })
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock
}));

import { parseApk, pickFiles } from "./tauri";

describe("parseApk", () => {
  it("invokes tauri parse command and returns normalized envelope", async () => {
    invokeMock.mockClear();
    const result = await parseApk("D:/tmp/demo.apk");
    expect(result.requestedPath).toBe("D:/tmp/demo.apk");
    expect(result.envelope.success).toBe(true);
    expect(result.envelope.data.packageName).toBe("com.example.app");
    expect(invokeMock).toHaveBeenCalledWith("parse_apk", { filePath: "D:/tmp/demo.apk" });
  });
});

describe("pickFiles", () => {
  it("returns only non-empty string paths", async () => {
    invokeMock.mockClear();
    const files = await pickFiles();
    expect(files).toEqual(["D:/tmp/a.apk", "D:/tmp/b.aab"]);
    expect(invokeMock).toHaveBeenCalledWith("pick_files");
  });
});
