export const WARNING_LABELS: Record<string, string> = {
  CHANNEL_NOT_FOUND: "未检测到渠道信息",
  ICON_NOT_FOUND: "未找到可提取图标",
  APP_NAME_UNRESOLVED: "应用名资源未能解析",
  SIGNATURE_PARTIAL: "签名信息为尽力解析，可能不完整",
  SIGNATURE_BLOCK_DETECTED_UNPARSED: "检测到 APK 签名块，但未完成完整解析",
  MANIFEST_BINARY_PARTIAL: "Manifest 为二进制且解析信息不完整"
};

export function toWarningLabel(code: string): string {
  return WARNING_LABELS[code] ?? `未知警告（${code}）`;
}
