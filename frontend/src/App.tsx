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
import { FileTab, TabStatus } from "./types/tab";
import { renderCopyJson, renderCopyText } from "./utils/copy";
import { createTabsFromPaths, ParseJob } from "./utils/workspace";

const EMPTY_TEXT = "无数据";
const MAX_TABS = 10;
const SECTION_PADDING = 0.72;
const COMPACT_LIST_ITEM_SX = { py: 0.02, minHeight: 22 };
const DIAGNOSTIC_WARNING_CODES = new Set([
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
  const [isParsing, setIsParsing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [toast, setToast] = useState<ToastState>({
    open: false,
    message: "",
    severity: "info"
  });
  const [resolvedIconUrl, setResolvedIconUrl] = useState("");
  const [showDiagnostics, setShowDiagnostics] = useState(false);
  const [signerExpanded, setSignerExpanded] = useState(false);
  const tabsRef = useRef<FileTab[]>([]);

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

  const addFiles = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) {
        return;
      }

      const currentTabs = tabsRef.current;
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
      tabsRef.current = nextTabs;
      setTabs(nextTabs);
      setActiveTabId((prev) => prev ?? createdTabs[0].id);
      if (jobs.length > 0) {
        setParseQueue((prev) => [...prev, ...jobs]);
      }
    },
    [showToast]
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

    setTabs((prev) => {
      const index = prev.findIndex((item) => item.id === activeTabId);
      const next = prev.filter((item) => item.id !== activeTabId);
      tabsRef.current = next;

      setActiveTabId(() => {
        if (next.length === 0) {
          return null;
        }
        const fallbackIndex = Math.min(index, next.length - 1);
        return next[fallbackIndex].id;
      });

      return next;
    });

    setParseQueue((prev) => prev.filter((job) => job.id !== activeTabId));
  }

  function closeOtherTabs() {
    if (!activeTabId) {
      return;
    }
    setTabs((prev) => {
      const next = prev.filter((item) => item.id === activeTabId);
      tabsRef.current = next;
      return next;
    });
    setParseQueue((prev) => prev.filter((job) => job.id === activeTabId));
  }

  function clearAllTabs() {
    tabsRef.current = [];
    setTabs([]);
    setActiveTabId(null);
    setParseQueue([]);
  }

  function retryCurrent() {
    if (!activeTab || activeTab.ext !== "apk") {
      return;
    }

    setTabs((prev) => {
      const next: FileTab[] = prev.map((item): FileTab =>
        item.id === activeTab.id
          ? {
              ...item,
              status: "pending",
              localError: null
            }
          : item
      );
      tabsRef.current = next;
      return next;
    });
    setParseQueue((prev) => [...prev, { id: activeTab.id, path: activeTab.path }]);
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
    if (isParsing || parseQueue.length === 0) {
      return;
    }

    const currentJob = parseQueue[0];
    setParseQueue((prev) => prev.slice(1));
    setIsParsing(true);

    setTabs((prev) => {
      const next: FileTab[] = prev.map((item): FileTab =>
        item.id === currentJob.id
          ? {
              ...item,
              status: "parsing",
              localError: null
            }
          : item
      );
      tabsRef.current = next;
      return next;
    });

    void parseApk(currentJob.path)
      .then((result) => {
        setTabs((prev) => {
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
          tabsRef.current = next;
          return next;
        });
      })
      .catch((error) => {
        setTabs((prev) => {
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
          tabsRef.current = next;
          return next;
        });
      })
      .finally(() => {
        setIsParsing(false);
      });
  }, [isParsing, parseQueue]);

  useEffect(() => {
    if (!activeTabId && tabs.length > 0) {
      setActiveTabId(tabs[0].id);
      return;
    }

    if (activeTabId && !tabs.some((item) => item.id === activeTabId)) {
      setActiveTabId(tabs[0]?.id ?? null);
    }
  }, [tabs, activeTabId]);

  const activeData = activeTab?.envelope?.data ?? null;
  const rawIconUrl = activeData?.iconUrl || "";
  const iconAvailable = Boolean(resolvedIconUrl);
  const hasSignaturePartialRisk =
    activeTab?.envelope?.warnings.includes("SIGNATURE_PARTIAL") ||
    activeTab?.envelope?.warnings.includes("SIGNATURE_BLOCK_DETECTED_UNPARSED");
  const allWarnings = activeTab?.envelope?.warnings ?? [];
  const iconPickedWarning = allWarnings.find((item) => isIconPickedWarning(item)) ?? "";
  const iconResolvedSuccessfully = Boolean(rawIconUrl && resolvedIconUrl);
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
  const signerDefaultExpanded =
    Boolean(activeData && activeData.signers.length > 0) && !hasSignaturePartialRisk;

  useEffect(() => {
    let active = true;

    async function resolveIcon() {
      if (!rawIconUrl) {
        if (active) {
          setResolvedIconUrl("");
        }
        return;
      }

      if (!rawIconUrl.startsWith("file://")) {
        if (active) {
          setResolvedIconUrl(rawIconUrl);
        }
        return;
      }

      try {
        const filePath = toLocalFilePath(rawIconUrl);
        if (!filePath) {
          if (active) {
            setResolvedIconUrl("");
          }
          return;
        }
        const dataUrl = await readIconDataUrl(filePath);
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
                label={<TabLabel name={tab.name} status={tab.status} />}
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

        {!activeTab ? (
          <Paper variant="outlined" sx={{ p: 1.2 }}>
            <Typography variant="body2" color="text.secondary">
              暂无标签。请拖入或选择 APK/AAB 文件。
            </Typography>
          </Paper>
        ) : (
          <>
            <Paper variant="outlined" sx={{ p: 0.7 }}>
              <Stack direction="row" spacing={0.75} alignItems="center" justifyContent="space-between">
                <Stack direction="row" spacing={0.75} alignItems="center" sx={{ minWidth: 0 }}>
                  <Chip size="small" label={statusLabel(activeTab.status)} color={statusColor(activeTab.status)} />
                  <Tooltip title={activeTab.path}>
                    <Typography variant="caption" sx={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                      {activeTab.path}
                    </Typography>
                  </Tooltip>
                </Stack>
                <Stack direction="row" spacing={0.55} alignItems="center">
                  {iconAvailable ? (
                    <Box
                      component="img"
                      src={resolvedIconUrl}
                      alt="icon"
                      sx={{ width: 40, height: 40, borderRadius: 1, border: "1px solid", borderColor: "divider", objectFit: "contain" }}
                    />
                  ) : (
                    <ImageNotSupportedIcon sx={{ fontSize: 24, color: "text.secondary" }} />
                  )}
                  {iconAvailable && activeTab.ext === "apk" && (
                    <Button size="small" variant="outlined" startIcon={<DownloadIcon />} onClick={() => onDownloadIcon(rawIconUrl, activeTab.name)}>
                      导出
                    </Button>
                  )}
                  {activeTab.ext === "apk" && activeTab.status === "error" && (
                    <Button size="small" variant="outlined" onClick={retryCurrent}>
                      重试
                    </Button>
                  )}
                </Stack>
              </Stack>
            </Paper>

            {activeTab.ext === "aab" ? (
              <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                <Alert severity="info">占位：AAB 暂不解析，已保留路径与标签。</Alert>
              </Paper>
            ) : activeTab.status === "parsing" ? (
              <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                <Stack direction="row" spacing={0.8} alignItems="center">
                  <CircularProgress size={16} />
                  <Typography variant="body2">正在按队列顺序解析该 APK...</Typography>
                </Stack>
              </Paper>
            ) : activeTab.status === "error" && !activeData ? (
              <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                <Alert severity="error">{activeTab.localError || "解析失败"}</Alert>
              </Paper>
            ) : (
              activeData && (
                <Box
                  sx={{
                    display: "grid",
                    gap: 0.55,
                    gridTemplateColumns: { xs: "repeat(2, minmax(0, 1fr))", lg: "repeat(12, minmax(0, 1fr))" }
                  }}
                >
                  <Box sx={{ gridColumn: { xs: "span 1", lg: "span 4" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="caption" sx={{ fontWeight: 700 }}>
                        基础信息
                      </Typography>
                      <List dense sx={{ py: 0 }}>
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText
                            primary="包名"
                            secondary={activeData.packageName || EMPTY_TEXT}
                            primaryTypographyProps={{ variant: "caption" }}
                            secondaryTypographyProps={{ variant: "body2" }}
                          />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText
                            primary="应用名"
                            secondary={activeData.appName || EMPTY_TEXT}
                            primaryTypographyProps={{ variant: "caption" }}
                            secondaryTypographyProps={{ variant: "body2" }}
                          />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText
                            primary="渠道"
                            secondary={activeData.channel || "unknown"}
                            primaryTypographyProps={{ variant: "caption" }}
                            secondaryTypographyProps={{ variant: "body2" }}
                          />
                        </ListItem>
                      </List>
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 1", lg: "span 4" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="caption" sx={{ fontWeight: 700 }}>
                        版本信息
                      </Typography>
                      <List dense sx={{ py: 0 }}>
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="minSdkVersion" secondary={activeData.minSdkVersion} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="targetSdkVersion" secondary={activeData.targetSdkVersion} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="compileSdkVersion" secondary={activeData.compileSdkVersion ?? "null"} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="versionCode" secondary={activeData.versionCode} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="versionName" secondary={activeData.versionName ?? "null"} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                        </ListItem>
                      </List>
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 2", lg: "span 4" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="caption" sx={{ fontWeight: 700 }}>
                        ABI / 警告
                      </Typography>
                      <Stack spacing={0.45} sx={{ mt: 0.4 }}>
                        <Typography variant="caption" color="text.secondary">
                          图标来源
                        </Typography>
                        {iconPickedWarning ? (
                          <Tooltip title={iconPickedWarning}>
                            <Chip size="small" label={toWarningLabel(iconPickedWarning)} color="info" variant="outlined" />
                          </Tooltip>
                        ) : (
                          <Typography variant="body2" color="text.secondary">
                            {EMPTY_TEXT}
                          </Typography>
                        )}
                        <Divider />
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

                  <Box sx={{ gridColumn: { xs: "span 2", lg: "span 6" }, minWidth: 0 }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="caption" sx={{ fontWeight: 700 }}>
                        权限列表
                      </Typography>
                      {activeData.permissions.length === 0 ? (
                        <Typography variant="body2" color="text.secondary" sx={{ mt: 0.35 }}>
                          {EMPTY_TEXT}
                        </Typography>
                      ) : (
                        <List dense sx={{ py: 0, maxHeight: 240, overflowY: "auto" }}>
                          {activeData.permissions.map((permission) => (
                            <ListItem key={permission} sx={COMPACT_LIST_ITEM_SX}>
                              <ListItemText primary={permission} primaryTypographyProps={{ variant: "caption" }} />
                            </ListItem>
                          ))}
                        </List>
                      )}
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "span 2", lg: "span 6" }, minWidth: 0 }}>
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
                        {activeData.signers.length === 0 ? "签名：无可用信息" : hasSignaturePartialRisk ? "签名：解析不完整" : `签名：${activeData.signers.length} 个`}
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
                                      <ListItemText primary="certSha256" secondary={signer.certSha256 || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                                    </ListItem>
                                    <Divider component="li" />
                                    <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                      <ListItemText primary="issuer" secondary={signer.issuer || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
                                    </ListItem>
                                    <Divider component="li" />
                                    <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                      <ListItemText primary="subject" secondary={signer.subject || EMPTY_TEXT} primaryTypographyProps={{ variant: "caption" }} secondaryTypographyProps={{ variant: "body2" }} />
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
              )
            )}
          </>
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

function TabLabel({ name, status }: { name: string; status: TabStatus }) {
  return (
    <Stack direction="row" spacing={0.4} alignItems="center">
      <StatusIcon status={status} />
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

export default App;
