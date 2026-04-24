import { FileTab } from "../types/tab";

export interface ParseJob {
  id: string;
  path: string;
}

export interface AddFilesSummary {
  duplicateCount: number;
  unsupportedCount: number;
  droppedByLimit: number;
}

export interface AddFilesResult {
  createdTabs: FileTab[];
  jobs: ParseJob[];
  summary: AddFilesSummary;
}

export function inferFileExt(path: string): "apk" | "aab" | "other" {
  const lower = path.toLowerCase();
  if (lower.endsWith(".apk")) {
    return "apk";
  }
  if (lower.endsWith(".aab")) {
    return "aab";
  }
  return "other";
}

export function getFileName(path: string): string {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return slash >= 0 ? path.slice(slash + 1) : path;
}

export function createTabsFromPaths(
  incomingPaths: string[],
  existingTabs: FileTab[],
  maxTabs: number,
  now = Date.now()
): AddFilesResult {
  const existingPathSet = new Set(existingTabs.map((item) => item.path.toLowerCase()));
  const createdTabs: FileTab[] = [];
  const jobs: ParseJob[] = [];
  const summary: AddFilesSummary = {
    duplicateCount: 0,
    unsupportedCount: 0,
    droppedByLimit: 0
  };

  incomingPaths.forEach((path, index) => {
    const key = path.toLowerCase();
    if (existingPathSet.has(key)) {
      summary.duplicateCount += 1;
      return;
    }
    existingPathSet.add(key);

    const ext = inferFileExt(path);
    if (ext === "other") {
      summary.unsupportedCount += 1;
      return;
    }

    if (existingTabs.length + createdTabs.length >= maxTabs) {
      summary.droppedByLimit += 1;
      return;
    }

    const id = `${now}-${index}-${path}`;
    const tab: FileTab = {
      id,
      name: getFileName(path),
      path,
      ext,
      status: ext === "aab" ? "placeholder" : "pending",
      envelope: null,
      localError: ext === "aab" ? "AAB 当前仅占位，暂不解析。" : null,
      createdAt: now
    };

    createdTabs.push(tab);
    if (ext === "apk") {
      jobs.push({ id, path });
    }
  });

  return { createdTabs, jobs, summary };
}
