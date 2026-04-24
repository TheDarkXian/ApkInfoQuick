import { ApkInfoEnvelope } from "./apk";

export type FileExt = "apk" | "aab";

export type TabStatus = "pending" | "parsing" | "success" | "error" | "placeholder";

export interface FileTab {
  id: string;
  name: string;
  path: string;
  ext: FileExt;
  status: TabStatus;
  envelope: ApkInfoEnvelope | null;
  localError: string | null;
  createdAt: number;
}

