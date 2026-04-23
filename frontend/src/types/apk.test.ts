import { describe, expect, it } from "vitest";
import { normalizeEnvelope } from "./apk";

describe("normalizeEnvelope", () => {
  it("fills required fallbacks when fields are missing", () => {
    const normalized = normalizeEnvelope({});

    expect(normalized.success).toBe(false);
    expect(normalized.data.versionName).toBeNull();
    expect(normalized.data.compileSdkVersion).toBeNull();
    expect(normalized.data.channel).toBe("unknown");
    expect(normalized.data.permissions).toEqual([]);
    expect(normalized.data.abis).toEqual([]);
    expect(normalized.data.signers).toEqual([]);
    expect(normalized.warnings).toEqual([]);
  });

  it("keeps valid envelope payload fields", () => {
    const normalized = normalizeEnvelope({
      success: true,
      data: {
        packageName: "com.example.demo",
        appName: "Demo",
        iconUrl: "file:///tmp/icon.png",
        minSdkVersion: 21,
        targetSdkVersion: 34,
        compileSdkVersion: 34,
        versionCode: 10203,
        versionName: "1.2.3",
        permissions: ["android.permission.INTERNET"],
        signers: [
          {
            scheme: "v2+v3",
            certSha256: "AA",
            issuer: "CN=A",
            subject: "CN=B",
            validFrom: "2024-01-01T00:00:00Z",
            validTo: "2034-01-01T00:00:00Z"
          }
        ],
        abis: ["arm64-v8a"],
        channel: "huawei"
      },
      warnings: ["FIELD_PARTIAL_IMPLEMENTATION"],
      errorCode: null,
      errorMessage: null
    });

    expect(normalized.success).toBe(true);
    expect(normalized.data.packageName).toBe("com.example.demo");
    expect(normalized.data.versionName).toBe("1.2.3");
    expect(normalized.data.permissions).toEqual(["android.permission.INTERNET"]);
    expect(normalized.data.signers).toHaveLength(1);
    expect(normalized.warnings).toEqual(["FIELD_PARTIAL_IMPLEMENTATION"]);
  });
});
