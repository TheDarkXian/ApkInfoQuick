import { describe, expect, it } from "vitest";
import { isIconPickedWarning, toWarningLabel, WARNING_LABELS } from "./warnings";

describe("warning labels", () => {
  it("maps known warning codes to chinese labels", () => {
    expect(toWarningLabel("CHANNEL_NOT_FOUND")).toBe("未检测到渠道信息");
    expect(toWarningLabel("SIGNATURE_PARTIAL")).toBe("签名信息为尽力解析，可能不完整");
    expect(toWarningLabel("AAPT_NOT_FOUND_FALLBACK_USED")).toBe("未找到 aapt，已使用内置解析兜底");
    expect(toWarningLabel("AAB_CONVERTED_BY_BUNDLETOOL")).toBe("AAB 已通过 bundletool 转换为 universal APK");
    expect(toWarningLabel("APP_NAME_PICKED_AAPT_LABEL")).toBe("应用名来源：aapt label");
  });

  it("returns fallback text for unknown warning code", () => {
    expect(toWarningLabel("UNKNOWN_CODE")).toBe("未知警告（UNKNOWN_CODE）");
  });

  it("maps icon picked strategy warning", () => {
    expect(toWarningLabel("ICON_PICKED_RESOURCE_ID_ARSC")).toBe("图标来源：resources.arsc 反查");
    expect(toWarningLabel("ICON_PICKED_AAPT_BADGING")).toBe("图标来源：aapt badging");
    expect(isIconPickedWarning("ICON_PICKED_RESOURCE_ID_ARSC")).toBe(true);
  });

  it("keeps all expected keys", () => {
    expect(Object.keys(WARNING_LABELS).sort()).toEqual(
      [
        "APP_NAME_UNRESOLVED",
        "APP_NAME_PICKED_AAPT_LABEL",
        "APP_NAME_PICKED_RESOURCE_ID",
        "APP_NAME_PICKED_STRING_REF",
        "AAPT_BADGING_FAILED_FALLBACK_USED",
        "AAPT_NOT_FOUND_FALLBACK_USED",
        "AAB_BUNDLETOOL_FAILED",
        "AAB_BUNDLETOOL_NOT_FOUND",
        "AAB_CONVERTED_BY_BUNDLETOOL",
        "AAB_JAVA_NOT_FOUND",
        "AAB_UNIVERSAL_APK_NOT_FOUND",
        "CHANNEL_NOT_FOUND",
        "ICON_ADAPTIVE_XML_UNRESOLVED",
        "ICON_CANDIDATES_EMPTY",
        "ICON_MANIFEST_REF_UNRESOLVED",
        "ICON_NOT_FOUND",
        "ICON_PICKED_ADAPTIVE_XML",
        "ICON_PICKED_AAPT_BADGING",
        "ICON_PICKED_AAPT_XMLTREE",
        "ICON_PICKED_HEURISTIC_FALLBACK",
        "ICON_PICKED_MANIFEST_PATH",
        "ICON_PICKED_RESOURCE_ID_ARSC",
        "ICON_PICKED_ROUND_ICON",
        "ICON_RESOURCE_ID_UNRESOLVED",
        "MANIFEST_BINARY_PARTIAL",
        "SIGNATURE_BLOCK_DETECTED_UNPARSED",
        "SIGNATURE_PARTIAL"
      ].sort()
    );
  });
});
