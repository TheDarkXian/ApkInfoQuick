import { ApkInfoEnvelope } from "./apk";

export type FileExt = "apk" | "aab";

export type TabStatus = "pending" | "parsing" | "success" | "error" | "placeholder";
export type SignerStatus = "idle" | "pending" | "parsing" | "success" | "error";

export interface FileTab {
  id: string;
  name: string;
  path: string;
  ext: FileExt;
  status: TabStatus;
  envelope: ApkInfoEnvelope | null;
  localError: string | null;
  signerStatus: SignerStatus;
  signerError: string | null;
  createdAt: number;
}
