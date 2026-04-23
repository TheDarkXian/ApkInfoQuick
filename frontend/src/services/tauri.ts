import { invoke } from "@tauri-apps/api/core";
import { ApkInfoEnvelope, normalizeEnvelope } from "../types/apk";

export interface ParseResult {
  envelope: ApkInfoEnvelope;
  requestedPath: string;
}

export async function parseApk(filePath: string): Promise<ParseResult> {
  const raw = await invoke<unknown>("parse_apk", { filePath });
  const envelope = normalizeEnvelope(raw);
  return { envelope, requestedPath: filePath };
}
