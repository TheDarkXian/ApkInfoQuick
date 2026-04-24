import copyTemplate from "../../templates/copy-text.template.txt?raw";
import { FileTab } from "../types/tab";

function listToLines(value: string[]): string {
  if (value.length === 0) {
    return "-";
  }
  return value.map((item) => `- ${item}`).join("\n");
}

function signersToLines(tab: FileTab): string {
  const signers = tab.envelope?.data.signers ?? [];
  if (signers.length === 0) {
    return "-";
  }

  return signers
    .map((signer, index) =>
      [
        `Signer #${index + 1}`,
        `  scheme: ${signer.scheme || "-"}`,
        `  certSha256: ${signer.certSha256 || "-"}`,
        `  issuer: ${signer.issuer || "-"}`,
        `  subject: ${signer.subject || "-"}`,
        `  validFrom: ${signer.validFrom || "-"}`,
        `  validTo: ${signer.validTo || "-"}`
      ].join("\n")
    )
    .join("\n");
}

function getFieldMap(tab: FileTab): Record<string, string> {
  const data = tab.envelope?.data;
  const warnings = tab.envelope?.warnings ?? [];

  return {
    file_name: tab.name,
    path: tab.path,
    ext: tab.ext,
    status: tab.status,
    packname: data?.packageName ?? "-",
    product_name: data?.appName ?? "-",
    app_name: data?.appName ?? "-",
    channel: data?.channel ?? "-",
    min_sdk_version: data?.minSdkVersion?.toString() ?? "-",
    target_sdk_version: data?.targetSdkVersion?.toString() ?? "-",
    compile_sdk_version:
      data?.compileSdkVersion === null || data?.compileSdkVersion === undefined
        ? "null"
        : String(data.compileSdkVersion),
    version_code: data?.versionCode?.toString() ?? "-",
    version_name: data?.versionName ?? "null",
    permissions: listToLines(data?.permissions ?? []),
    abis: listToLines(data?.abis ?? []),
    signers: signersToLines(tab),
    warnings: listToLines(warnings),
    error_code: tab.envelope?.errorCode ?? "-",
    error_message: tab.localError || tab.envelope?.errorMessage || "-"
  };
}

export function renderCopyText(tab: FileTab): string {
  const map = getFieldMap(tab);
  return copyTemplate
    .split("\n")
    .filter((line) => !line.trim().startsWith("# "))
    .join("\n")
    .replace(/#([a-z0-9_]+)#/gi, (_, key: string) => map[key] ?? "");
}

export function renderCopyJson(tab: FileTab): string {
  if (tab.ext === "aab" && tab.status === "placeholder") {
    return JSON.stringify(
      {
        path: tab.path,
        name: tab.name,
        status: "placeholder",
        reason: "AAB_PLACEHOLDER"
      },
      null,
      2
    );
  }

  return JSON.stringify(
    {
      path: tab.path,
      name: tab.name,
      status: tab.status,
      envelope: tab.envelope,
      localError: tab.localError
    },
    null,
    2
  );
}
