import { useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import CloudUploadIcon from "@mui/icons-material/CloudUpload";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";
import DataObjectIcon from "@mui/icons-material/DataObject";
import DeleteSweepIcon from "@mui/icons-material/DeleteSweep";
import FilterAltOffIcon from "@mui/icons-material/FilterAltOff";
import HighlightOffIcon from "@mui/icons-material/HighlightOff";
import DownloadIcon from "@mui/icons-material/Download";
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
import { parseApk, pickFiles } from "./services/tauri";
import { FileTab, TabStatus } from "./types/tab";
import { renderCopyJson, renderCopyText } from "./utils/copy";
import { createTabsFromPaths, ParseJob } from "./utils/workspace";

const EMPTY_TEXT = "无数据";
const MAX_TABS = 10;
const SECTION_PADDING = 1.25;
const COMPACT_LIST_ITEM_SX = { py: 0.2, minHeight: 28 };

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

  const activeTab = useMemo(
    () => tabs.find((item) => item.id === activeTabId) ?? null,
    [tabs, activeTabId]
  );

  function showToast(message: string, severity: ToastSeverity = "info") {
    setToast({ open: true, message, severity });
  }

  function closeToast() {
    setToast((prev) => ({ ...prev, open: false }));
  }

  function addFiles(paths: string[]) {
    if (paths.length === 0) {
      return;
    }

    const { createdTabs, jobs, summary } = createTabsFromPaths(paths, tabs, MAX_TABS);

    if (summary.duplicateCount > 0) {
      showToast("部分文件已存在，已自动忽略重复项。", "info");
    }

    if (summary.unsupportedCount > 0) {
      showToast(`已忽略 ${summary.unsupportedCount} 个非 APK/AAB 文件。`, "warning");
    }

    if (summary.droppedByLimit > 0) {
      showToast(`最多支持 ${MAX_TABS} 个标签，已忽略 ${summary.droppedByLimit} 个文件。`, "warning");
    }

    if (createdTabs.length === 0) {
      if (tabs.length >= MAX_TABS) {
        showToast(`最多支持 ${MAX_TABS} 个标签。`, "warning");
      }
      return;
    }

    setTabs((prev) => [...prev, ...createdTabs]);
    setActiveTabId((prev) => prev ?? createdTabs[0].id);
    setParseQueue((prev) => [...prev, ...jobs]);
  }

  async function onPickFile() {
    try {
      const paths = await pickFiles();
      addFiles(paths);
    } catch (error) {
      showToast(error instanceof Error ? error.message : "打开文件选择器失败", "error");
    }
  }

  function onDownloadIcon(iconUrl: string) {
    if (!iconUrl) {
      return;
    }

    const link = document.createElement("a");
    link.href = iconUrl;
    link.download = "app-icon.png";
    link.click();
  }

  function closeCurrentTab() {
    if (!activeTabId) {
      return;
    }

    setTabs((prev) => {
      const index = prev.findIndex((item) => item.id === activeTabId);
      const next = prev.filter((item) => item.id !== activeTabId);

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
    setTabs((prev) => prev.filter((item) => item.id === activeTabId));
    setParseQueue((prev) => prev.filter((job) => job.id === activeTabId));
  }

  function clearAllTabs() {
    setTabs([]);
    setActiveTabId(null);
    setParseQueue([]);
  }

  function retryCurrent() {
    if (!activeTab || activeTab.ext !== "apk") {
      return;
    }

    setTabs((prev) =>
      prev.map((item) =>
        item.id === activeTab.id
          ? {
              ...item,
              status: "pending",
              localError: null
            }
          : item
      )
    );
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
  }, [tabs]);

  useEffect(() => {
    if (isParsing || parseQueue.length === 0) {
      return;
    }

    const currentJob = parseQueue[0];
    setParseQueue((prev) => prev.slice(1));
    setIsParsing(true);

    setTabs((prev) =>
      prev.map((item) =>
        item.id === currentJob.id
          ? {
              ...item,
              status: "parsing",
              localError: null
            }
          : item
      )
    );

    void parseApk(currentJob.path)
      .then((result) => {
        setTabs((prev) => {
          if (!prev.some((item) => item.id === currentJob.id)) {
            return prev;
          }
          return prev.map((item) => {
            if (item.id !== currentJob.id) {
              return item;
            }
            return {
              ...item,
              envelope: result.envelope,
              status: result.envelope.success ? "success" : "error",
              localError: result.envelope.success
                ? null
                : result.envelope.errorMessage || "解析失败"
            };
          });
        });
      })
      .catch((error) => {
        setTabs((prev) => {
          if (!prev.some((item) => item.id === currentJob.id)) {
            return prev;
          }
          return prev.map((item) => {
            if (item.id !== currentJob.id) {
              return item;
            }
            return {
              ...item,
              status: "error",
              localError: error instanceof Error ? error.message : "解析请求失败"
            };
          });
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
  const iconUrl = activeData?.iconUrl || "";
  const iconAvailable = Boolean(iconUrl);
  const hasSignaturePartialRisk =
    activeTab?.envelope?.warnings.includes("SIGNATURE_PARTIAL") ||
    activeTab?.envelope?.warnings.includes("SIGNATURE_BLOCK_DETECTED_UNPARSED");

  return (
    <Container maxWidth="xl" sx={{ py: 1.25 }}>
      <Stack spacing={1}>
        <Paper
          variant="outlined"
          sx={{
            p: 1.25,
            borderStyle: "dashed",
            borderWidth: 1.5,
            borderColor: dragging ? "primary.main" : "divider",
            backgroundColor: dragging ? "rgba(11, 87, 208, 0.05)" : "background.paper",
            transition: "all 0.2s ease"
          }}
        >
          <Stack direction={{ xs: "column", md: "row" }} spacing={1} alignItems="center" justifyContent="space-between">
            <Stack direction="row" spacing={1} alignItems="center">
              <CloudUploadIcon color="primary" sx={{ fontSize: 22 }} />
              <Typography variant="body2">拖拽 APK/AAB 到窗口，或手动选择（最多 10 个标签）</Typography>
            </Stack>
            <Stack direction="row" spacing={0.75}>
              <Button size="small" variant="contained" onClick={onPickFile}>
                选择文件
              </Button>
              <Button size="small" variant="outlined" startIcon={<ContentCopyIcon />} onClick={copyCurrentText}>
                复制文本
              </Button>
              <Button size="small" variant="outlined" startIcon={<DataObjectIcon />} onClick={copyCurrentJson}>
                复制 JSON
              </Button>
              <Button size="small" variant="outlined" startIcon={<HighlightOffIcon />} onClick={closeCurrentTab}>
                关闭当前
              </Button>
              <Button size="small" variant="outlined" startIcon={<FilterAltOffIcon />} onClick={closeOtherTabs}>
                关闭其他
              </Button>
              <Button size="small" color="error" variant="outlined" startIcon={<DeleteSweepIcon />} onClick={clearAllTabs}>
                清空
              </Button>
            </Stack>
          </Stack>
        </Paper>

        <Paper variant="outlined" sx={{ p: 0.5 }}>
          <Tabs
            value={activeTabId ?? false}
            onChange={(_, value) => setActiveTabId(value)}
            variant="scrollable"
            scrollButtons="auto"
            sx={{ minHeight: 32 }}
          >
            {tabs.map((tab) => (
              <Tab
                key={tab.id}
                value={tab.id}
                label={<TabLabel name={tab.name} status={tab.status} />}
                sx={{
                  minHeight: 32,
                  textTransform: "none",
                  py: 0,
                  px: 1
                }}
              />
            ))}
          </Tabs>
        </Paper>

        {!activeTab ? (
          <Paper variant="outlined" sx={{ p: 2 }}>
            <Typography variant="body2" color="text.secondary">
              暂无标签。请拖入或选择 APK/AAB 文件。
            </Typography>
          </Paper>
        ) : (
          <>
            <Paper variant="outlined" sx={{ p: 1 }}>
              <Stack direction="row" spacing={1} alignItems="center" justifyContent="space-between">
                <Stack direction="row" spacing={1} alignItems="center" sx={{ minWidth: 0 }}>
                  <Chip size="small" label={statusLabel(activeTab.status)} color={statusColor(activeTab.status)} />
                  <Tooltip title={activeTab.path}>
                    <Typography variant="body2" sx={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                      {activeTab.path}
                    </Typography>
                  </Tooltip>
                </Stack>
                <Stack direction="row" spacing={0.75} alignItems="center">
                  {iconAvailable ? (
                    <Box
                      component="img"
                      src={iconUrl}
                      alt="icon"
                      sx={{ width: 24, height: 24, borderRadius: 1, border: "1px solid", borderColor: "divider" }}
                    />
                  ) : (
                    <ImageNotSupportedIcon sx={{ fontSize: 20, color: "text.secondary" }} />
                  )}
                  {iconAvailable && activeTab.ext === "apk" && (
                    <Button size="small" variant="text" startIcon={<DownloadIcon />} onClick={() => onDownloadIcon(iconUrl)}>
                      导出图标
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
                <Stack direction="row" spacing={1} alignItems="center">
                  <CircularProgress size={18} />
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
                    gap: 0.8,
                    gridTemplateColumns: { xs: "1fr", lg: "repeat(12, 1fr)" }
                  }}
                >
                  <Box sx={{ gridColumn: { xs: "1 / -1", lg: "span 4" } }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="subtitle2" gutterBottom>
                        基础信息
                      </Typography>
                      <List dense sx={{ py: 0 }}>
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="包名" secondary={activeData.packageName || EMPTY_TEXT} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="应用名" secondary={activeData.appName || EMPTY_TEXT} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="渠道" secondary={activeData.channel || "unknown"} />
                        </ListItem>
                      </List>
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "1 / -1", lg: "span 4" } }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="subtitle2" gutterBottom>
                        版本信息
                      </Typography>
                      <List dense sx={{ py: 0 }}>
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="minSdkVersion" secondary={activeData.minSdkVersion} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="targetSdkVersion" secondary={activeData.targetSdkVersion} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText
                            primary="compileSdkVersion"
                            secondary={activeData.compileSdkVersion ?? "null"}
                          />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="versionCode" secondary={activeData.versionCode} />
                        </ListItem>
                        <Divider component="li" />
                        <ListItem sx={COMPACT_LIST_ITEM_SX}>
                          <ListItemText primary="versionName" secondary={activeData.versionName ?? "null"} />
                        </ListItem>
                      </List>
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "1 / -1", lg: "span 4" } }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="subtitle2" gutterBottom>
                        ABI / 警告
                      </Typography>
                      <Stack spacing={0.75}>
                        <Typography variant="caption" color="text.secondary">
                          ABI
                        </Typography>
                        {activeData.abis.length === 0 ? (
                          <Typography variant="body2" color="text.secondary">
                            {EMPTY_TEXT}
                          </Typography>
                        ) : (
                          <Stack direction="row" spacing={0.5} useFlexGap flexWrap="wrap">
                            {activeData.abis.map((abi) => (
                              <Chip key={abi} size="small" label={abi} />
                            ))}
                          </Stack>
                        )}
                        <Divider />
                        <Typography variant="caption" color="text.secondary">
                          Warnings
                        </Typography>
                        {activeTab.envelope?.warnings.length ? (
                          <Stack direction="row" spacing={0.5} useFlexGap flexWrap="wrap">
                            {activeTab.envelope.warnings.map((warning) => (
                              <Chip key={warning} size="small" label={warning} color="warning" variant="outlined" />
                            ))}
                          </Stack>
                        ) : (
                          <Typography variant="body2" color="text.secondary">
                            {EMPTY_TEXT}
                          </Typography>
                        )}
                      </Stack>
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "1 / -1", lg: "span 6" } }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="subtitle2" gutterBottom>
                        权限列表
                      </Typography>
                      {activeData.permissions.length === 0 ? (
                        <Typography variant="body2" color="text.secondary">
                          {EMPTY_TEXT}
                        </Typography>
                      ) : (
                        <List dense sx={{ py: 0 }}>
                          {activeData.permissions.map((permission) => (
                            <ListItem key={permission} sx={COMPACT_LIST_ITEM_SX}>
                              <ListItemText primary={permission} />
                            </ListItem>
                          ))}
                        </List>
                      )}
                    </Paper>
                  </Box>

                  <Box sx={{ gridColumn: { xs: "1 / -1", lg: "span 6" } }}>
                    <Paper variant="outlined" sx={{ p: SECTION_PADDING }}>
                      <Typography variant="subtitle2" gutterBottom>
                        签名信息
                      </Typography>
                      {hasSignaturePartialRisk && (
                        <Alert severity="warning" sx={{ mb: 1 }}>
                          当前签名信息为尽力解析，部分证书元数据可能不完整。
                        </Alert>
                      )}
                      {activeData.signers.length === 0 ? (
                        <Typography variant="body2" color="text.secondary">
                          {EMPTY_TEXT}
                        </Typography>
                      ) : (
                        <Stack spacing={0.75}>
                          {activeData.signers.map((signer, index) => (
                            <Paper key={`${signer.certSha256}-${index}`} variant="outlined" sx={{ p: 0.75 }}>
                              <Typography variant="caption" sx={{ fontWeight: 600 }}>
                                签名者 #{index + 1}
                              </Typography>
                              <List dense sx={{ py: 0 }}>
                                <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                  <ListItemText primary="scheme" secondary={signer.scheme || EMPTY_TEXT} />
                                </ListItem>
                                <Divider component="li" />
                                <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                  <ListItemText
                                    primary="certSha256"
                                    secondary={signer.certSha256 || EMPTY_TEXT}
                                  />
                                </ListItem>
                                <Divider component="li" />
                                <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                  <ListItemText primary="issuer" secondary={signer.issuer || EMPTY_TEXT} />
                                </ListItem>
                                <Divider component="li" />
                                <ListItem sx={COMPACT_LIST_ITEM_SX}>
                                  <ListItemText primary="subject" secondary={signer.subject || EMPTY_TEXT} />
                                </ListItem>
                              </List>
                            </Paper>
                          ))}
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
    <Stack direction="row" spacing={0.5} alignItems="center">
      <StatusIcon status={status} />
      <Typography variant="caption" sx={{ maxWidth: 180 }} noWrap>
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

export default App;
