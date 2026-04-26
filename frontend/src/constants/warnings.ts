export const WARNING_LABELS: Record<string, string> = {
  AAPT_NOT_FOUND_FALLBACK_USED: "未找到 aapt，已使用内置解析兜底",
  AAPT_BADGING_FAILED_FALLBACK_USED: "aapt 解析失败，已使用内置解析兜底",
  AAB_CONVERTED_BY_BUNDLETOOL: "AAB 已通过 bundletool 转换为 universal APK",
  AAB_BUNDLETOOL_NOT_FOUND: "未找到 bundletool，无法解析 AAB",
  AAB_JAVA_NOT_FOUND: "未找到 Java，无法运行 bundletool",
  AAB_BUNDLETOOL_FAILED: "bundletool 转换 AAB 失败",
  AAB_UNIVERSAL_APK_NOT_FOUND: "bundletool 输出中未找到 universal APK",
  CHANNEL_NOT_FOUND: "未检测到渠道信息",
  ICON_NOT_FOUND: "未找到可提取图标",
  ICON_MANIFEST_REF_UNRESOLVED: "Manifest 图标引用未能解析到资源文件",
  ICON_RESOURCE_ID_UNRESOLVED: "资源 ID 图标引用未能反查到资源名",
  ICON_ADAPTIVE_XML_UNRESOLVED: "Adaptive Icon XML 解析失败或未命中图层资源",
  ICON_CANDIDATES_EMPTY: "未找到可用图标候选",
  ICON_PICKED_MANIFEST_PATH: "图标来源：Manifest 直接路径",
  ICON_PICKED_ROUND_ICON: "图标来源：Manifest RoundIcon",
  ICON_PICKED_RESOURCE_ID_ARSC: "图标来源：resources.arsc 反查",
  ICON_PICKED_ADAPTIVE_XML: "图标来源：Adaptive XML 图层",
  ICON_PICKED_HEURISTIC_FALLBACK: "图标来源：启发式兜底",
  ICON_PICKED_AAPT_BADGING: "图标来源：aapt badging",
  ICON_PICKED_AAPT_XMLTREE: "图标来源：aapt xmltree",
  APP_NAME_PICKED_STRING_REF: "应用名来源：字符串资源引用",
  APP_NAME_PICKED_RESOURCE_ID: "应用名来源：resources.arsc 反查",
  APP_NAME_PICKED_AAPT_LABEL: "应用名来源：aapt label",
  APP_NAME_UNRESOLVED: "应用名资源未能解析",
  SIGNATURE_PARTIAL: "签名信息为尽力解析，可能不完整",
  SIGNATURE_BLOCK_DETECTED_UNPARSED: "检测到 APK 签名块，但未完成完整解析",
  MANIFEST_BINARY_PARTIAL: "Manifest 为二进制且解析信息不完整"
};

export function toWarningLabel(code: string): string {
  if (code.startsWith("ICON_PICKED_")) {
    return WARNING_LABELS[code] ?? `图标来源：${code.replace("ICON_PICKED_", "")}`;
  }
  if (code.startsWith("APP_NAME_PICKED_")) {
    return WARNING_LABELS[code] ?? `应用名来源：${code.replace("APP_NAME_PICKED_", "")}`;
  }
  return WARNING_LABELS[code] ?? `未知警告（${code}）`;
}

export function isIconPickedWarning(code: string): boolean {
  return code.startsWith("ICON_PICKED_");
}
