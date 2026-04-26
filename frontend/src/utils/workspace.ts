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

function createUniqueId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `tab-${Date.now()}-${Math.random().toString(16).slice(2)}`;
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
  now = Date.now(),
  idFactory: () => string = createUniqueId
): AddFilesResult {
  const existingPathSet = new Set(existingTabs.map((item) => item.path.toLowerCase()));
  const createdTabs: FileTab[] = [];
  const jobs: ParseJob[] = [];
  const summary: AddFilesSummary = {
    duplicateCount: 0,
    unsupportedCount: 0,
    droppedByLimit: 0
  };

  incomingPaths.forEach((path) => {
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

    const id = idFactory();
    const tab: FileTab = {
      id,
      name: getFileName(path),
      path,
      ext,
      status: "pending",
      envelope: null,
      localError: null,
      createdAt: now
    };

    createdTabs.push(tab);
    jobs.push({ id, path });
  });

  return { createdTabs, jobs, summary };
}
