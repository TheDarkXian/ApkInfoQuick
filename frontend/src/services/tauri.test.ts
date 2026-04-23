import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => ({
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
  }))
}));

import { parseApk } from "./tauri";

describe("parseApk", () => {
  it("invokes tauri parse command and returns normalized envelope", async () => {
    const result = await parseApk("D:/tmp/demo.apk");
    expect(result.requestedPath).toBe("D:/tmp/demo.apk");
    expect(result.envelope.success).toBe(true);
    expect(result.envelope.data.packageName).toBe("com.example.app");
  });
});

