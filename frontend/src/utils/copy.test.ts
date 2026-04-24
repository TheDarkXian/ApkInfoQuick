import { describe, expect, it } from "vitest";
import { renderCopyJson, renderCopyText } from "./copy";
import { FileTab } from "../types/tab";

function makeTab(overrides: Partial<FileTab> = {}): FileTab {
  return {
    id: "1",
    name: "demo.apk",
    path: "D:/tmp/demo.apk",
    ext: "apk",
    status: "success",
    localError: null,
    createdAt: 1,
    envelope: {
      success: true,
      errorCode: null,
      errorMessage: null,
      warnings: ["CHANNEL_NOT_FOUND"],
      data: {
        packageName: "com.example.demo",
        appName: "Demo",
        iconUrl: "file:///tmp/icon.png",
        minSdkVersion: 21,
        targetSdkVersion: 34,
        compileSdkVersion: 34,
        versionCode: 1001,
        versionName: "1.0.1",
        permissions: ["android.permission.INTERNET"],
        signers: [],
        abis: ["arm64-v8a"],
        channel: "unknown"
      }
    },
    ...overrides
  };
}

describe("renderCopyText", () => {
  it("renders placeholders and keeps configured field order", () => {
    const output = renderCopyText(makeTab());
    expect(output).toContain("FileName: demo.apk");
    expect(output).toContain("FilePath: D:/tmp/demo.apk");
    expect(output).toContain("PackageName: com.example.demo");
    expect(output).toContain("Permissions:");
    expect(output).toContain("- android.permission.INTERNET");
  });

  it("does not leak template comment lines", () => {
    const output = renderCopyText(makeTab());
    expect(output).not.toContain("# Copy Text Template");
    expect(output).not.toContain("# Placeholder format");
  });
});

describe("renderCopyJson", () => {
  it("returns placeholder payload for aab tabs", () => {
    const json = renderCopyJson(
      makeTab({
        ext: "aab",
        status: "placeholder",
        path: "D:/tmp/demo.aab",
        name: "demo.aab"
      })
    );
    const parsed = JSON.parse(json) as { reason: string; status: string; path: string };
    expect(parsed.status).toBe("placeholder");
    expect(parsed.reason).toBe("AAB_PLACEHOLDER");
    expect(parsed.path).toBe("D:/tmp/demo.aab");
  });
});

