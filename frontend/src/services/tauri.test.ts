import { describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(async (cmd: string) => {
    if (cmd === "pick_files") {
      return ["D:/tmp/a.apk", "D:/tmp/b.aab", "", "   "] as unknown;
    }
    if (cmd === "read_icon_data_url") {
      return "data:image/png;base64,AAAA" as unknown;
    }
    if (cmd === "export_icon_with_dialog") {
      return "D:/export/app-icon.png" as unknown;
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

import { exportIconWithDialog, parseApk, pickFiles, readIconDataUrl } from "./tauri";

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

describe("readIconDataUrl", () => {
  it("returns data url from tauri command", async () => {
    invokeMock.mockClear();
    const dataUrl = await readIconDataUrl("D:/tmp/icon.png");
    expect(dataUrl).toBe("data:image/png;base64,AAAA");
    expect(invokeMock).toHaveBeenCalledWith("read_icon_data_url", { filePath: "D:/tmp/icon.png" });
  });
});

describe("exportIconWithDialog", () => {
  it("returns save path from tauri command", async () => {
    invokeMock.mockClear();
    const savedPath = await exportIconWithDialog("D:/tmp/icon.png", "demo-icon.png");
    expect(savedPath).toBe("D:/export/app-icon.png");
    expect(invokeMock).toHaveBeenCalledWith("export_icon_with_dialog", {
      sourceFilePath: "D:/tmp/icon.png",
      suggestedFileName: "demo-icon.png"
    });
  });
});
