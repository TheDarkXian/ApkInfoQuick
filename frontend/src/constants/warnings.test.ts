import { describe, expect, it } from "vitest";
import { toWarningLabel, WARNING_LABELS } from "./warnings";

describe("warning labels", () => {
  it("maps known warning codes to chinese labels", () => {
    expect(toWarningLabel("CHANNEL_NOT_FOUND")).toBe("未检测到渠道信息");
    expect(toWarningLabel("SIGNATURE_PARTIAL")).toBe("签名信息为尽力解析，可能不完整");
  });

  it("returns fallback text for unknown warning code", () => {
    expect(toWarningLabel("UNKNOWN_CODE")).toBe("未知警告（UNKNOWN_CODE）");
  });

  it("keeps all expected keys", () => {
    expect(Object.keys(WARNING_LABELS).sort()).toEqual(
      [
        "APP_NAME_UNRESOLVED",
        "CHANNEL_NOT_FOUND",
        "ICON_ADAPTIVE_XML_UNRESOLVED",
        "ICON_CANDIDATES_EMPTY",
        "ICON_MANIFEST_REF_UNRESOLVED",
        "ICON_NOT_FOUND",
        "ICON_RESOURCE_ID_UNRESOLVED",
        "MANIFEST_BINARY_PARTIAL",
        "SIGNATURE_BLOCK_DETECTED_UNPARSED",
        "SIGNATURE_PARTIAL"
      ].sort()
    );
  });
});
