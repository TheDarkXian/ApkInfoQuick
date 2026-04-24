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

export async function pickFiles(): Promise<string[]> {
  const picked = await invoke<string[] | null>("pick_files");
  if (!Array.isArray(picked)) {
    return [];
  }
  return picked.filter((item) => typeof item === "string" && item.trim().length > 0);
}
