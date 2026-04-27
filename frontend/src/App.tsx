import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import CloudUploadIcon from "@mui/icons-material/CloudUpload";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import DataObjectIcon from "@mui/icons-material/DataObject";
import DeleteSweepIcon from "@mui/icons-material/DeleteSweep";
import FilterAltOffIcon from "@mui/icons-material/FilterAltOff";
import HighlightOffIcon from "@mui/icons-material/HighlightOff";
import DownloadIcon from "@mui/icons-material/Download";
import ExpandLessIcon from "@mui/icons-material/ExpandLess";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import ImageNotSupportedIcon from "@mui/icons-material/ImageNotSupported";
import HourglassTopIcon from "@mui/icons-material/HourglassTop";
import ErrorOutlineIcon from "@mui/icons-material/ErrorOutline";
import CheckCircleOutlineIcon from "@mui/icons-material/CheckCircleOutline";
import LabelImportantOutlineIcon from "@mui/icons-material/LabelImportantOutline";
import InsertDriveFileOutlinedIcon from "@mui/icons-material/InsertDriveFileOutlined";
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Container,
  Divider,
  List,
  ListItem,
  ListItemText,
  Paper,
  Snackbar,
  Stack,
  Tab,
  Tabs,
  Tooltip,
  Typography
} from "@mui/material";
import { exportIconWithDialog, parseApk, pickFiles, readIconDataUrl } from "./services/tauri";
import { isIconPickedWarning, toWarningLabel } from "./constants/warnings";
import { ApkInfoData, Signer } from "./types/apk";
import { FileTab, TabStatus } from "./types/tab";
import { renderCopyJson, renderCopyText } from "./utils/copy";
import { createTabsFromPaths, ParseJob } from "./utils/workspace";

const EMPTY_TEXT = "无数据";
const MAX_TABS = 10;
const SECTION_PADDING = 0.55;
const COMPACT_LIST_ITEM_SX = { py: 0, minHeight: 20 };
const LONG_VALUE_TYPOGRAPHY_PROPS = {
  variant: "body2" as const,
  sx: {
    maxWidth: "100%",
    overflowWrap: "anywhere",
    wordBreak: "break-all",
    lineHeight: 1.25
  }
};
const HASH_VALUE_TYPOGRAPHY_PROPS = {
  ...LONG_VALUE_TYPOGRAPHY_PROPS,
  sx: {
    ...LONG_VALUE_TYPOGRAPHY_PROPS.sx,
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
  }
};
const DIAGNOSTIC_WARNING_CODES = new Set([
  "AAPT_NOT_FOUND_FALLBACK_USED",
  "AAPT_BADGING_FAILED_FALLBACK_USED",
  "ICON_MANIFEST_REF_UNRESOLVED",
  "ICON_RESOURCE_ID_UNRESOLVED",
  "ICON_ADAPTIVE_XML_UNRESOLVED",
  "ICON_CANDIDATES_EMPTY",
  "APP_NAME_UNRESOLVED",
  "SIGNATURE_PARTIAL",
  "SIGNATURE_BLOCK_DETECTED_UNPARSED"
]);

type ToastSeverity = "success" | "info" | "warning" | "error";

interface ToastState {
  open: boolean;
  message: string;
  severity: ToastSeverity;
}

function App() {
  const [tabs, setTabs] = useState<FileTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [parseQueue, setParseQueue] = useState<ParseJob[]>([]);
  const [currentParseJob, setCurrentParseJob] = useState<ParseJob | null>(null);
  const [dragging, setDragging] = useState(false);
  const [toast, setToast] = useState<ToastState>({
    open: false,
    message: "",
    severity: "info"
  });
  const [tabIconUrls, setTabIconUrls] = useState<Record<string, string | null>>({});
  const [resolvedIconUrl, setResolvedIconUrl] = useState("");
  const [showDiagnostics, setShowDiagnostics] = useState(false);
  const [signerExpanded, setSignerExpanded] = useState(false);
  const tabsRef = useRef<FileTab[]>([]);
  const parseQueueRef = useRef<ParseJob[]>([]);
  const currentParseJobRef = useRef<ParseJob | null>(null);

  const activeTab = useMemo(
    () => tabs.find((item) => item.id === activeTabId) ?? null,
    [tabs, activeTabId]
  );

  const showToast = useCallback((message: string, severity: ToastSeverity = "info") => {
    setToast({ open: true, message, severity });
  }, []);

  function closeToast() {
    setToast((prev) => ({ ...prev, open: false }));
  }

  const setTabsSynced = useCallback((updater: (prev: FileTab[]) => FileTab[]) => {
    const next = updater(tabsRef.current);
    tabsRef.current = next;
    setTabs(next);
    return next;
  }, []);

  const setParseQueueSynced = useCallback((updater: (prev: ParseJob[]) => ParseJob[]) => {
    const next = updater(parseQueueRef.current);
    parseQueueRef.current = next;
    setParseQueue(next);
    return next;
  }, []);

  const setCurrentParseJobSynced = useCallback((job: ParseJob | null) => {
    currentParseJobRef.current = job;
    setCurrentParseJob(job);
  }, []);

  const addFiles = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) {
        return;
      }

      const currentTabs = tabsRef.current;
      const wasBusy = Boolean(currentParseJobRef.current) || parseQueueRef.current.length > 0;
      const { createdTabs, jobs, summary } = createTabsFromPaths(paths, currentTabs, MAX_TABS);

      if (summary.unsupportedCount > 0) {
        showToast(`已忽略 ${summary.unsupportedCount} 个非 APK/AAB 文件。`, "warning");
      }

      if (summary.droppedByLimit > 0) {
        showToast(`最多支持 ${MAX_TABS} 个标签，已忽略 ${summary.droppedByLimit} 个文件。`, "warning");
      }

      if (createdTabs.length === 0) {
        if (currentTabs.length >= MAX_TABS) {
          showToast(`最多支持 ${MAX_TABS} 个标签。`, "warning");
        }
        return;
      }

      const nextTabs = [...currentTabs, ...createdTabs];
      setTabsSynced(() => nextTabs);
      setActiveTabId((prev) => prev ?? createdTabs[0].id);
      if (jobs.length > 0) {
        setParseQueueSynced((prev) => [...prev, ...jobs]);
        if (wasBusy) {
          showToast(`已加入解析队列：${jobs.length} 个文件。`, "info");
        }
      }
    },
    [setParseQueueSynced, setTabsSynced, showToast]
  );

  async function onPickFile() {
    try {
      const paths = await pickFiles();
      addFiles(paths);
    } catch (error) {
      showToast(error instanceof Error ? error.message : "打开文件选择器失败", "error");
    }
  }

  async function onDownloadIcon(rawIconUrl: string, fileName: string) {
    const sourceFilePath = toLocalFilePath(rawIconUrl);
    if (!sourceFilePath) {
      showToast("当前图标不可导出。", "warning");
      return;
    }

    try {
      const fallbackName = fileName.replace(/\.(apk|aab)$/i, "");
      const ext = sourceFilePath.toLowerCase().endsWith(".webp") ? "webp" : "png";
      const suggestedFileName = `${fallbackName || "app"}-icon.${ext}`;
      const savedPath = await exportIconWithDialog(sourceFilePath, suggestedFileName);
      if (!savedPath) {
        showToast("已取消导出。", "info");
        return;
      }
      showToast(`图标已导出到：${savedPath}`, "success");
    } catch (error) {
      showToast(error instanceof Error ? error.message : "图标导出失败。", "error");
    }
  }

  function closeCurrentTab() {
    if (!activeTabId) {
      return;
    }

    const closingTabId = activeTabId;
    setTabsSynced((prev) => {
      const index = prev.findIndex((item) => item.id === closingTabId);
      const next = prev.filter((item) => item.id !== closingTabId);

      setActiveTabId(() => {
        if (next.length === 0) {
          return null;
        }
        const fallbackIndex = Math.min(index, next.length - 1);
        return next[fallbackIndex].id;
      });

      return next;
    });

    setParseQueueSynced((prev) => prev.filter((job) => job.id !== closingTabId));
    setTabIconUrls((prev) => omitIconCache(prev, [closingTabId]));
  }

  function closeOtherTabs() {
    if (!activeTabId) {
      return;
    }
    setTabsSynced((prev) => {
      const next = prev.filter((item) => item.id === activeTabId);
      return next;
    });
    setParseQueueSynced((prev) => prev.filter((job) => job.id === activeTabId));
    setTabIconUrls((prev) => {
      const kept = activeTabId ? prev[activeTabId] : undefined;
      return activeTabId && kept !== undefined ? { [activeTabId]: kept } : {};
    });
  }

  function clearAllTabs() {
    setTabsSynced(() => []);
    setActiveTabId(null);
    setParseQueueSynced(() => []);
    setTabIconUrls({});
  }

  function retryCurrent() {
    if (!activeTab) {
      return;
    }

    setTabsSynced((prev) => {
      const next: FileTab[] = prev.map((item): FileTab =>
        item.id === activeTab.id
          ? {
              ...item,
              status: "pending",
              localError: null
            }
          : item
      );
      return next;
    });
    setTabIconUrls((prev) => omitIconCache(prev, [activeTab.id]));
    setParseQueueSynced((prev) => [...prev, { id: activeTab.id, path: activeTab.path }]);
  }

  async function copyCurrentText() {
    if (!activeTab) {
      showToast("当前没有可复制的标签。", "warning");
      return;
    }

    try {
      await navigator.clipboard.writeText(renderCopyText(activeTab));
      showToast("已复制当前标签文本。", "success");
    } catch {
      showToast("复制文本失败。", "error");
    }
  }

  async function copyCurrentJson() {
    if (!activeTab) {
      showToast("当前没有可复制的标签。", "warning");
      return;
    }

    try {
      await navigator.clipboard.writeText(renderCopyJson(activeTab));
      showToast("已复制当前标签 JSON。", "success");
    } catch {
      showToast("复制 JSON 失败。", "error");
    }
  }

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

  useEffect(() => {
    parseQueueRef.current = parseQueue;
  }, [parseQueue]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void getCurrentWindow()
      .onDragDropEvent((event) => {
        const { payload } = event;

        if (payload.type === "enter") {
          setDragging(true);
          return;
        }

        if (payload.type === "leave") {
          setDragging(false);
          return;
        }

        if (payload.type === "drop") {
          setDragging(false);
          addFiles(payload.paths);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        // ignore in non-tauri environment
      });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [addFiles]);

  useEffect(() => {
    if (currentParseJob || parseQueue.length === 0) {
      return;
    }

    const currentJob = parseQueueRef.current[0];
    if (!currentJob) {
      return;
    }
    setParseQueueSynced((prev) => prev.slice(1));
    setCurrentParseJobSynced(currentJob);

    setTabsSynced((prev) => {
      const next: FileTab[] = prev.map((item): FileTab =>
        item.id === currentJob.id
          ? {
              ...item,
              status: "parsing",
              localError: null
            }
          : item
      );
      return next;
    });
    setTabIconUrls((prev) => omitIconCache(prev, [currentJob.id]));

    void parseApk(currentJob.path)
      .then((result) => {
        setTabsSynced((prev) => {
          if (!prev.some((item) => item.id === currentJob.id)) {
            return prev;
          }
          const next: FileTab[] = prev.map((item): FileTab => {
            if (item.id !== currentJob.id) {
              return item;
            }
            return {
              ...item,
              envelope: result.envelope,
              status: result.envelope.success ? "success" : "error",
              localError: result.envelope.success ? null : result.envelope.errorMessage || "解析失败"
            };
          });
          return next;
        });
      })
      .catch((error) => {
        setTabsSynced((prev) => {
          if (!prev.some((item) => item.id === currentJob.id)) {
            return prev;
          }
          const next: FileTab[] = prev.map((item): FileTab => {
            if (item.id !== currentJob.id) {
              return item;
            }
            return {
              ...item,
              status: "error",
              localError: error instanceof Error ? error.message : "解析请求失败"
            };
          });
          return next;
        });
      })
      .finally(() => {
        setCurrentParseJobSynced(null);
      });
  }, [currentParseJob, parseQueue, setCurrentParseJobSynced, setParseQueueSynced, setTabsSynced]);

  useEffect(() => {
    if (!activeTabId && tabs.length > 0) {
      setActiveTabId(tabs[0].id);
      return;
    }

    if (activeTabId && !tabs.some((item) => item.id === activeTabId)) {
      setActiveTabId(tabs[0]?.id ?? null);
    }
  }, [tabs, activeTabId]);

  useEffect(() => {
    const targets = tabs.filter(
      (tab) => tab.status === "success" && Boolean(tab.envelope?.data?.iconUrl) && tabIconUrls[tab.id] === undefined
    );
    if (targets.length === 0) {
      return;
    }

    setTabIconUrls((prev) => {
      const next = { ...prev };
      for (const tab of targets) {
        if (next[tab.id] === undefined) {
          next[tab.id] = null;
        }
      }
      return next;
    });

    for (const tab of targets) {
      const iconUrl = tab.envelope?.data?.iconUrl || "";
      void resolveIconDataUrl(iconUrl)
        .then((dataUrl) => {
          const currentTab = tabsRef.current.find((item) => item.id === tab.id);
          if (currentTab?.status !== "success" || currentTab.envelope?.data?.iconUrl !== iconUrl) {
            return;
          }
          setTabIconUrls((prev) => ({ ...prev, [tab.id]: dataUrl }));
        })
        .catch(() => {
          if (!tabsRef.current.some((item) => item.id === tab.id)) {
            return;
          }
          setTabIconUrls((prev) => ({ ...prev, [tab.id]: null }));
        });
    }
  }, [tabs, tabIconUrls]);

  const activeData = activeTab?.envelope?.data ?? null;
  const queueTabs = tabs.filter((tab) => tab.status === "pending" || tab.status === "parsing" || tab.status === "error");
  const rawIconUrl = activeData?.iconUrl || "";
  const activeResolvedIconUrl = (activeTabId ? tabIconUrls[activeTabId] : null) || resolvedIconUrl;
  const iconAvailable = Boolean(activeResolvedIconUrl);
  const hasSignaturePartialRisk = Boolean(
    activeTab?.envelope?.warnings.includes("SIGNATURE_PARTIAL") ||
      activeTab?.envelope?.warnings.includes("SIGNATURE_BLOCK_DETECTED_UNPARSED")
  );
  const allWarnings = activeTab?.envelope?.warnings ?? [];
  const iconPickedWarning = allWarnings.find((item) => isIconPickedWarning(item)) ?? "";
  const iconResolvedSuccessfully = Boolean(rawIconUrl && activeResolvedIconUrl);
  const diagnosticWarnings = allWarnings.filter((item) => {
    if (isIconPickedWarning(item)) {
      return false;
    }
    if (!DIAGNOSTIC_WARNING_CODES.has(item)) {
      return false;
    }
    if (!item.startsWith("ICON_")) {
      return true;
    }
    return iconResolvedSuccessfully;
  });
  const primaryWarnings = allWarnings.filter((item) => !isIconPickedWarning(item) && !diagnosticWarnings.includes(item));
  const signerDefaultExpanded = Boolean(activeData?.signers.some(hasMeaningfulSignerValue));

  useEffect(() => {
    let active = true;

    async function resolveIcon() {
      if (!rawIconUrl) {
        if (active) {
          setResolvedIconUrl("");
        }
        return;
      }

      try {
        const dataUrl = await resolveIconDataUrl(rawIconUrl);
        if (active) {
          setResolvedIconUrl(dataUrl ?? "");
        }
      } catch {
        if (active) {
          setResolvedIconUrl("");
        }
      }
    }

    void resolveIcon();
    return () => {
      active = false;
    };
  }, [rawIconUrl]);

  useEffect(() => {
    setShowDiagnostics(false);
    setSignerExpanded(signerDefaultExpanded);
  }, [activeTabId, signerDefaultExpanded]);

  return (
    <Container maxWidth={false} sx={{ py: 0.75, px: 1 }}>
      <Stack spacing={0.75}>
        <Paper
          variant="outlined"
          sx={{
            p: 0.85,
            borderStyle: "dashed",
            borderWidth: 1.2,
            borderColor: dragging ? "primary.main" : "divider",
            backgroundColor: dragging ? "rgba(11, 87, 208, 0.05)" : "background.paper",
            transition: "all 0.2s ease"
          }}
        >
          <Stack direction={{ xs: "column", md: "row" }} spacing={0.75} alignItems="center" justifyContent="space-between">
            <Stack direction="row" spacing={0.75} alignItems="center" sx={{ minWidth: 0 }}>
              <CloudUploadIcon color="primary" sx={{ fontSize: 18 }} />
              <Typography variant="caption" noWrap>
                拖拽 APK/AAB 到窗口或选择文件（最多 10 个标签）
              </Typography>
            </Stack>
            <Stack direction="row" spacing={0.5} useFlexGap flexWrap="wrap" justifyContent="flex-end">
              <Button size="small" variant="contained" onClick={onPickFile}>
                选择
              </Button>
              <Button size="small" variant="outlined" startIcon={<ContentCopyIcon />} onClick={copyCurrentText}>
                文本
              </Button>
              <Button size="small" variant="outlined" startIcon={<DataObjectIcon />} onClick={copyCurrentJson}>
                JSON
              </Button>
              <Button size="small" variant="outlined" startIcon={<HighlightOffIcon />} onClick={closeCurrentTab}>
                关当前
              </Button>
              <Button size="small" variant="outlined" startIcon={<FilterAltOffIcon />} onClick={closeOtherTabs}>
                关其他
              </Button>
              <Button size="small" color="error" variant="outlined" startIcon={<DeleteSweepIcon />} onClick={clearAllTabs}>
                清空
              </Button>
            </Stack>
          </Stack>
        </Paper>

        <Paper variant="outlined" sx={{ p: 0.2 }}>
          <Tabs
            value={activeTabId ?? false}
            onChange={(_, value) => setActiveTabId(value)}
            variant="scrollable"
            scrollButtons="auto"
            sx={{ minHeight: 30 }}
          >
            {tabs.map((tab) => (
              <Tab
                key={tab.id}
                value={tab.id}
                label={<TabLabel name={tab.name} status={tab.status} iconSrc={tabIconUrls[tab.id] || ""} />}
                sx={{
                  minHeight: 30,
                  textTransform: "none",
                  py: 0,
                  px: 0.8
                }}
              />
            ))}
          </Tabs>
        </Paper>

        {queueTabs.length > 0 && (
          <TaskQueuePanel
            tabs={queueTabs}
            currentJobId={currentParseJob?.id ?? null}
            onActivate={(id) => setActiveTabId(id)}
          />
        )}

        {!activeTab ? (
          <Paper variant="outlined" sx={{ p: 1.2 }}>
            <Typography variant="body2" color="text.secondary">
              暂无标签。请拖入或选择 APK/AAB 文件。
            </Typography>
          </Paper>
        ) : (
          <Stack spacing={0.55}>
            {activeTab.status === "parsing" ? (
              <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                <Stack direction="row" spacing={0.8} alignItems="center">
                  <CircularProgress size={16} />
                  <Typography variant="body2">正在按队列顺序解析该文件...</Typography>
                </Stack>
              </Paper>
            ) : activeTab.status === "error" && !activeData ? (
              <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                <Alert severity="error">{activeTab.localError || "解析失败"}</Alert>
              </Paper>
            ) : (
              activeData && (
                <Stack spacing={0.55}>
                  <Box
                    sx={{
                      display: "grid",
                      gap: 0.55,
                      gridTemplateColumns: { xs: "repeat(2, minmax(0, 1fr))", lg: "repeat(12, minmax(0, 1fr))" }
                    }}
                  >
                  <Box sx={{ gridColumn: { xs: "span 1", lg: "span 2" }, minWidth: 0 }}>
                    <IconPanel
                      iconSrc={activeResolvedIconUrl}
                      iconPickedWarning={iconPickedWarning}
                      canExport={Boolean(rawIconUrl && iconAvailable)}
                      onExport={() => onDownloadIcon(rawIconUrl, activeTab.name)}
                    />
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 1", lg: "span 6" }, minWidth: 0 }}>
                    <CoreInfoPanel data={activeData} />
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 2", lg: "span 4" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="caption" sx={{ fontWeight: 700 }}>
                        ABI / 警告
                      </Typography>
                      <Stack spacing={0.45} sx={{ mt: 0.4 }}>
                        <Typography variant="caption" color="text.secondary">
                          ABI
                        </Typography>
                        {activeData.abis.length === 0 ? (
                          <Typography variant="body2" color="text.secondary">
                            {EMPTY_TEXT}
                          </Typography>
                        ) : (
                          <Stack direction="row" spacing={0.45} useFlexGap flexWrap="wrap">
                            {activeData.abis.map((abi) => (
                              <Chip key={abi} size="small" label={abi} />
                            ))}
                          </Stack>
                        )}
                        <Divider />
                        <Typography variant="caption" color="text.secondary">
                          警告
                        </Typography>
                        {primaryWarnings.length ? (
                          <Stack direction="row" spacing={0.45} useFlexGap flexWrap="wrap">
                            {primaryWarnings.map((warning) => (
                              <Tooltip key={warning} title={warning}>
                                <Chip size="small" label={toWarningLabel(warning)} color="warning" variant="outlined" />
                              </Tooltip>
                            ))}
                          </Stack>
                        ) : (
                          <Typography variant="body2" color="text.secondary">
                            {EMPTY_TEXT}
                          </Typography>
                        )}
                        {diagnosticWarnings.length > 0 && (
                          <>
                            <Divider />
                            <Stack direction="row" justifyContent="space-between" alignItems="center">
                              <Typography variant="caption" color="text.secondary">
                                详细诊断（{diagnosticWarnings.length}）
                              </Typography>
                              <Button
                                size="small"
                                variant="text"
                                onClick={() => setShowDiagnostics((prev) => !prev)}
                                endIcon={showDiagnostics ? <ExpandLessIcon fontSize="small" /> : <ExpandMoreIcon fontSize="small" />}
                              >
                                {showDiagnostics ? "收起" : "展开"}
                              </Button>
                            </Stack>
                            {showDiagnostics && (
                              <Stack direction="row" spacing={0.45} useFlexGap flexWrap="wrap">
                                {diagnosticWarnings.map((warning) => (
                                  <Tooltip key={warning} title={warning}>
                                    <Chip size="small" label={toWarningLabel(warning)} color="default" variant="outlined" />
                                  </Tooltip>
                                ))}
                              </Stack>
                            )}
                          </>
                        )}
                      </Stack>
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 2", lg: "span 8" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="caption" sx={{ fontWeight: 700 }}>
                        权限列表
                      </Typography>
                      {activeData.permissions.length === 0 ? (
                        <Typography variant="body2" color="text.secondary" sx={{ mt: 0.35 }}>
                          {EMPTY_TEXT}
                        </Typography>
                      ) : (
                        <List dense sx={{ py: 0, maxHeight: 210, overflowY: "auto" }}>
                          {activeData.permissions.map((permission) => (
                            <ListItem key={permission} sx={COMPACT_LIST_ITEM_SX}>
                              <ListItemText primary={permission} primaryTypographyProps={{ variant: "caption" }} />
                            </ListItem>
                          ))}
                        </List>
                      )}
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 2", lg: "span 4" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Stack direction="row" justifyContent="space-between" alignItems="center">
                        <Typography variant="caption" sx={{ fontWeight: 700 }}>
                          签名信息
                        </Typography>
                        <Button
                          size="small"
                          variant="text"
                          onClick={() => setSignerExpanded((prev) => !prev)}
                          endIcon={signerExpanded ? <ExpandLessIcon fontSize="small" /> : <ExpandMoreIcon fontSize="small" />}
                        >
                          {signerExpanded ? "收起" : "展开"}
                        </Button>
                      </Stack>
                      <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.35 }}>
                        {getSignerSummary(activeData.signers, hasSignaturePartialRisk)}
                      </Typography>
                      {signerExpanded && (
                        <Stack spacing={0.55} sx={{ mt: 0.45 }}>
                          {hasSignaturePartialRisk && (
                            <Alert severity="warning" sx={{ mb: 0.3, py: 0 }}>
                              当前签名信息为尽力解析，部分证书元数据可能不完整。
                            </Alert>
                          )}
                          {activeData.signers.length === 0 ? (
                            <Typography variant="body2" color="text.secondary">
                              {EMPTY_TEXT}
                            </Typography>
                          ) : (
                            <Stack spacing={0.55}>
                              {activeData.signers.map((signer, index) => (
                                <Paper key={`${signer.certSha256}-${index}`} variant="outlined" sx={{ p: 0.5 }}>
                                  <Typography variant="caption" sx={{ fontWeight: 600 }}>
                                    签名者 #{index + 1}
                                  </Typography>
                                  <List dense sx={{ py: 0 }}>
                                    <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                      <ListItemText primary="scheme" secondary={signer.scheme || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                                    </ListItem>
                                    <Divider component="li" />
                                    <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                      <ListItemText primary="certSha256" secondary={signer.certSha256 || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={HASH_VALUE_TYPOGRAPHY_PROPS} />
                                    </ListItem>
                                    <Divider component="li" />
                                    <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                      <ListItemText primary="issuer" secondary={signer.issuer || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={LONG_VALUE_TYPOGRAPHY_PROPS} />
                                    </ListItem>
                                    <Divider component="li" />
                                    <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                      <ListItemText primary="subject" secondary={signer.subject || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={LONG_VALUE_TYPOGRAPHY_PROPS} />
                                    </ListItem>
                                  </List>
                                </Paper>
                              ))}
                            </Stack>
                          )}
                        </Stack>
                      )}
                    </Paper>
                  </Box>
                  </Box>
                </Stack>
              )
            )}
            <SourcePanel tab={activeTab} onRetry={retryCurrent} />
          </Stack>
        )}
      </Stack>

      <Snackbar
        open={toast.open}
        autoHideDuration={2200}
        onClose={closeToast}
        anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
      >
        <Alert severity={toast.severity} onClose={closeToast} variant="filled" sx={{ width: "100%" }}>
          {toast.message}
        </Alert>
      </Snackbar>
    </Container>
  );
}

function IconPanel({
  iconSrc,
  iconPickedWarning,
  canExport,
  onExport
}: {
  iconSrc: string;
  iconPickedWarning: string;
  canExport: boolean;
  onExport: () => void;
}) {
  return (
    <Paper variant="outlined" sx={{ p: SECTION_PADDING, height: "100%" }}>
      <Stack spacing={0.45} alignItems="center" sx={{ height: "100%" }}>
        <Typography variant="caption" sx={{ fontWeight: 700, alignSelf: "stretch" }}>
          图标
        </Typography>
        <Box
          sx={{
            width: { xs: 82, sm: 96 },
            height: { xs: 82, sm: 96 },
            borderRadius: 1.35,
            border: "1px solid",
            borderColor: "divider",
            backgroundColor: "rgba(15, 23, 42, 0.025)",
            display: "grid",
            placeItems: "center",
            overflow: "hidden"
          }}
        >
          {iconSrc ? (
            <Box component="img" src={iconSrc} alt="应用图标" sx={{ width: "86%", height: "86%", objectFit: "contain" }} />
          ) : (
            <ImageNotSupportedIcon sx={{ fontSize: 34, color: "text.secondary" }} />
          )}
        </Box>
        {iconPickedWarning ? (
          <Tooltip title={iconPickedWarning}>
            <Chip size="small" label={toWarningLabel(iconPickedWarning)} color="info" variant="outlined" sx={{ maxWidth: "100%" }} />
          </Tooltip>
        ) : (
          <Typography variant="caption" color="text.secondary">
            {iconSrc ? "来源未标记" : "未解析到图标"}
          </Typography>
        )}
        <Button size="small" variant="outlined" startIcon={<DownloadIcon />} disabled={!canExport} onClick={onExport} sx={{ mt: "auto" }}>
          导出
        </Button>
      </Stack>
    </Paper>
  );
}

function TaskQueuePanel({
  tabs,
  currentJobId,
  onActivate
}: {
  tabs: FileTab[];
  currentJobId: string | null;
  onActivate: (id: string) => void;
}) {
  const parsingTabs = tabs.filter((tab) => tab.status === "parsing" || tab.id === currentJobId);
  const pendingTabs = tabs.filter((tab) => tab.status === "pending");
  const errorTabs = tabs.filter((tab) => tab.status === "error");

  return (
    <Paper variant="outlined" sx={{ p: 0.5 }}>
      <Stack direction={{ xs: "column", md: "row" }} spacing={0.55} alignItems={{ xs: "stretch", md: "center" }}>
        <Stack direction="row" spacing={0.45} alignItems="center" sx={{ flex: "0 0 auto" }}>
          <HourglassTopIcon sx={{ fontSize: 15, color: parsingTabs.length ? "warning.main" : "text.secondary" }} />
          <Typography variant="caption" sx={{ fontWeight: 700 }}>
            解析队列
          </Typography>
          <Chip size="small" label={`进行中 ${parsingTabs.length}`} color={parsingTabs.length ? "warning" : "default"} />
          <Chip size="small" label={`等待 ${pendingTabs.length}`} />
          {errorTabs.length > 0 && <Chip size="small" label={`失败 ${errorTabs.length}`} color="error" variant="outlined" />}
        </Stack>

        <Stack direction="row" spacing={0.4} useFlexGap flexWrap="wrap" sx={{ minWidth: 0 }}>
          {parsingTabs.map((tab) => (
            <Tooltip key={`parsing-${tab.id}`} title={tab.path}>
              <Chip
                size="small"
                color="warning"
                variant="outlined"
                icon={<CircularProgress size={10} />}
                label={`解析中：${tab.name}`}
                onClick={() => onActivate(tab.id)}
                sx={{ maxWidth: 230 }}
              />
            </Tooltip>
          ))}
          {pendingTabs.slice(0, 6).map((tab) => (
            <Tooltip key={`pending-${tab.id}`} title={tab.path}>
              <Chip
                size="small"
                label={`等待：${tab.name}`}
                onClick={() => onActivate(tab.id)}
                sx={{ maxWidth: 210 }}
              />
            </Tooltip>
          ))}
          {pendingTabs.length > 6 && <Chip size="small" label={`+${pendingTabs.length - 6}`} />}
          {errorTabs.slice(0, 3).map((tab) => (
            <Tooltip key={`error-${tab.id}`} title={tab.localError || tab.path}>
              <Chip
                size="small"
                color="error"
                variant="outlined"
                label={`失败：${tab.name}`}
                onClick={() => onActivate(tab.id)}
                sx={{ maxWidth: 210 }}
              />
            </Tooltip>
          ))}
        </Stack>
      </Stack>
    </Paper>
  );
}

function CoreInfoPanel({ data }: { data: ApkInfoData }) {
  return (
    <Paper variant="outlined" sx={{ p: SECTION_PADDING, height: "100%" }}>
      <Typography variant="caption" sx={{ fontWeight: 700 }}>
        核心信息
      </Typography>
      <Box
        sx={{
          display: "grid",
          gap: "1px 8px",
          gridTemplateColumns: { xs: "1fr", sm: "repeat(2, minmax(0, 1fr))" },
          mt: 0.35
        }}
      >
        <DetailField label="包名" value={data.packageName || EMPTY_TEXT} wide />
        <DetailField label="应用名" value={data.appName || EMPTY_TEXT} />
        <DetailField label="渠道" value={data.channel || "unknown"} />
        <DetailField label="versionCode" value={data.versionCode} />
        <DetailField label="versionName" value={data.versionName ?? "null"} />
        <DetailField label="minSdk" value={data.minSdkVersion} />
        <DetailField label="targetSdk" value={data.targetSdkVersion} />
        <DetailField label="compileSdk" value={data.compileSdkVersion ?? "null"} />
      </Box>
    </Paper>
  );
}

function DetailField({ label, value, wide = false }: { label: string; value: string | number; wide?: boolean }) {
  return (
    <Box
      sx={{
        minWidth: 0,
        gridColumn: wide ? { xs: "span 1", sm: "span 2" } : "span 1",
        py: 0.22,
        borderBottom: "1px solid",
        borderColor: "divider"
      }}
    >
      <Typography variant="caption" color="text.secondary" sx={{ display: "block", lineHeight: 1.15 }}>
        {label}
      </Typography>
      <Typography variant="body2" sx={{ lineHeight: 1.28, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={String(value)}>
        {value}
      </Typography>
    </Box>
  );
}

function SourcePanel({ tab, onRetry }: { tab: FileTab; onRetry: () => void }) {
  return (
    <Paper variant="outlined" sx={{ p: 0.55 }}>
      <Stack direction="row" spacing={0.6} alignItems="center" justifyContent="space-between">
        <Stack direction="row" spacing={0.55} alignItems="center" sx={{ minWidth: 0 }}>
          <Typography variant="caption" sx={{ fontWeight: 700, flex: "0 0 auto" }}>
            文件来源
          </Typography>
          <Chip size="small" label={statusLabel(tab.status)} color={statusColor(tab.status)} />
          <Tooltip title={tab.path}>
            <Typography variant="caption" sx={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
              {tab.path}
            </Typography>
          </Tooltip>
        </Stack>
        {tab.status === "error" && (
          <Button size="small" variant="outlined" onClick={onRetry}>
            重试
          </Button>
        )}
      </Stack>
    </Paper>
  );
}

function TabLabel({ name, status, iconSrc }: { name: string; status: TabStatus; iconSrc?: string }) {
  return (
    <Stack direction="row" spacing={0.4} alignItems="center" sx={{ minWidth: 0 }}>
      {iconSrc && status === "success" ? (
        <Box
          sx={{
            position: "relative",
            width: 18,
            height: 18,
            flex: "0 0 auto"
          }}
        >
          <Box
            component="img"
            src={iconSrc}
            alt=""
            sx={{ width: 18, height: 18, borderRadius: 0.65, border: "1px solid", borderColor: "divider", objectFit: "contain" }}
          />
          <CheckCircleOutlineIcon
            sx={{
              position: "absolute",
              right: -5,
              bottom: -5,
              fontSize: 10,
              color: "success.main",
              backgroundColor: "background.paper",
              borderRadius: "50%"
            }}
          />
        </Box>
      ) : (
        <StatusIcon status={status} />
      )}
      <Typography variant="caption" sx={{ maxWidth: 170 }} noWrap>
        {name}
      </Typography>
    </Stack>
  );
}

function StatusIcon({ status }: { status: TabStatus }) {
  switch (status) {
    case "pending":
      return <LabelImportantOutlineIcon sx={{ fontSize: 14, color: "text.secondary" }} />;
    case "parsing":
      return <HourglassTopIcon sx={{ fontSize: 14, color: "warning.main" }} />;
    case "success":
      return <CheckCircleOutlineIcon sx={{ fontSize: 14, color: "success.main" }} />;
    case "error":
      return <ErrorOutlineIcon sx={{ fontSize: 14, color: "error.main" }} />;
    case "placeholder":
      return <InsertDriveFileOutlinedIcon sx={{ fontSize: 14, color: "info.main" }} />;
    default:
      return null;
  }
}

function statusLabel(status: TabStatus): string {
  switch (status) {
    case "pending":
      return "待解析";
    case "parsing":
      return "解析中";
    case "success":
      return "成功";
    case "error":
      return "失败";
    case "placeholder":
      return "占位";
    default:
      return "未知";
  }
}

function statusColor(status: TabStatus): "default" | "success" | "warning" | "error" | "info" {
  switch (status) {
    case "success":
      return "success";
    case "parsing":
      return "warning";
    case "error":
      return "error";
    case "placeholder":
      return "info";
    default:
      return "default";
  }
}

function hasMeaningfulSignerValue(signer: Signer): boolean {
  return [signer.scheme, signer.certSha256, signer.issuer, signer.subject, signer.validFrom, signer.validTo].some((value) =>
    isMeaningfulText(value)
  );
}

function isMeaningfulText(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return Boolean(normalized && normalized !== "unknown" && normalized !== EMPTY_TEXT.toLowerCase());
}

function getSignerSummary(signers: Signer[], hasPartialRisk: boolean): string {
  if (signers.length === 0 || !signers.some(hasMeaningfulSignerValue)) {
    return "签名：无可用信息";
  }
  return hasPartialRisk ? `签名：${signers.length} 个，部分字段未识别` : `签名：${signers.length} 个`;
}

function toLocalFilePath(iconUrl: string): string | null {
  if (!iconUrl.startsWith("file://")) {
    return null;
  }
  try {
    const parsed = new URL(iconUrl);
    const decoded = decodeURIComponent(parsed.pathname);
    if (/^\/[a-zA-Z]:\//.test(decoded)) {
      return decoded.slice(1);
    }
    return decoded;
  } catch {
    return null;
  }
}

async function resolveIconDataUrl(iconUrl: string): Promise<string | null> {
  if (!iconUrl) {
    return null;
  }
  if (!iconUrl.startsWith("file://")) {
    return iconUrl;
  }

  const filePath = toLocalFilePath(iconUrl);
  if (!filePath) {
    return null;
  }
  return readIconDataUrl(filePath);
}

function omitIconCache(cache: Record<string, string | null>, ids: string[]): Record<string, string | null> {
  if (ids.length === 0) {
    return cache;
  }
  const idSet = new Set(ids);
  return Object.fromEntries(Object.entries(cache).filter(([id]) => !idSet.has(id)));
}

export default App;
