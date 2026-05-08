import { invoke } from "@tauri-apps/api/core";
import { ApkInfoEnvelope, normalizeEnvelope, Signer } from "../types/apk";

export interface ParseResult {
  envelope: ApkInfoEnvelope;
  requestedPath: string;
}

export interface SignerParseResult {
  success: boolean;
  signers: Signer[];
  warnings: string[];
  errorCode: string | null;
  errorMessage: string | null;
  requestedPath: string;
}

export async function parseApk(filePath: string): Promise<ParseResult> {
  const raw = await invoke<unknown>("parse_apk", { filePath });
  const envelope = normalizeEnvelope(raw);
  return { envelope, requestedPath: filePath };
}

export async function parseSigners(filePath: string): Promise<SignerParseResult> {
  const raw = await invoke<unknown>("parse_signers", { filePath });
  const parsed = normalizeSignerParseResult(raw);
  return { ...parsed, requestedPath: filePath };
}

export async function pickFiles(): Promise<string[]> {
  const picked = await invoke<string[] | null>("pick_files");
  if (!Array.isArray(picked)) {
    return [];
  }
  return picked.filter((item) => typeof item === "string" && item.trim().length > 0);
}

export async function readIconDataUrl(filePath: string): Promise<string | null> {
  const raw = await invoke<string | null>("read_icon_data_url", { filePath });
  if (typeof raw !== "string" || !raw.startsWith("data:image/")) {
    return null;
  }
  return raw;
}

export async function exportIconWithDialog(sourceFilePath: string, suggestedFileName: string): Promise<string | null> {
  const raw = await invoke<string | null>("export_icon_with_dialog", {
    sourceFilePath,
    suggestedFileName
  });
  return typeof raw === "string" && raw.trim().length > 0 ? raw : null;
}

function normalizeSignerParseResult(input: unknown): Omit<SignerParseResult, "requestedPath"> {
  const raw = typeof input === "object" && input !== null ? (input as Record<string, unknown>) : {};
  const signers = Array.isArray(raw.signers)
    ? raw.signers.map((item): Signer => {
        const signer = typeof item === "object" && item !== null ? (item as Record<string, unknown>) : {};
        return {
          scheme: typeof signer.scheme === "string" ? signer.scheme : "",
          certSha256: typeof signer.certSha256 === "string" ? signer.certSha256 : "",
          issuer: typeof signer.issuer === "string" ? signer.issuer : "",
          subject: typeof signer.subject === "string" ? signer.subject : "",
          validFrom: typeof signer.validFrom === "string" ? signer.validFrom : "",
          validTo: typeof signer.validTo === "string" ? signer.validTo : ""
        };
      })
    : [];
  const warnings = Array.isArray(raw.warnings) ? raw.warnings.filter((item): item is string => typeof item === "string") : [];
  return {
    success: raw.success === true,
    signers,
    warnings,
    errorCode: typeof raw.errorCode === "string" ? raw.errorCode : null,
    errorMessage: typeof raw.errorMessage === "string" ? raw.errorMessage : null
  };
}
