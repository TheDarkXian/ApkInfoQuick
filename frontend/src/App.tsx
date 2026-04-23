import { ChangeEvent, DragEvent, useMemo, useRef, useState } from "react";
import CloudUploadIcon from "@mui/icons-material/CloudUpload";
import DownloadIcon from "@mui/icons-material/Download";
import ReplayIcon from "@mui/icons-material/Replay";
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
  Stack,
  Typography
} from "@mui/material";
import { parseApk } from "./services/tauri";
import { ApkInfoEnvelope } from "./types/apk";

type UiState = "idle" | "loading" | "ready" | "error";

const EMPTY_TEXT = "No data";

function App() {
  const [uiState, setUiState] = useState<UiState>("idle");
  const [envelope, setEnvelope] = useState<ApkInfoEnvelope | null>(null);
  const [lastFilePath, setLastFilePath] = useState<string | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const canRetry = useMemo(() => Boolean(lastFilePath), [lastFilePath]);

  async function startParse(path: string) {
    setUiState("loading");
    setLocalError(null);
    setEnvelope(null);

    try {
      const result = await parseApk(path);
      setEnvelope(result.envelope);
      setLastFilePath(result.requestedPath);
      setUiState(result.envelope.success ? "ready" : "error");
    } catch (error) {
      setUiState("error");
      setLocalError(error instanceof Error ? error.message : "Parse request failed");
    }
  }

  function getFilePath(file: File): string {
    const fileWithPath = file as File & { path?: string };
    if (typeof fileWithPath.path === "string" && fileWithPath.path.length > 0) {
      return fileWithPath.path;
    }
    return "";
  }

  function isApk(fileName: string): boolean {
    return fileName.toLowerCase().endsWith(".apk");
  }

  function processFiles(files: FileList | File[]) {
    const list = Array.from(files);
    if (list.length === 0) {
      return;
    }

    const file = list[0];
    if (!isApk(file.name)) {
      setUiState("error");
      setEnvelope(null);
      setLocalError("Only .apk files are supported");
      return;
    }

    const filePath = getFilePath(file);
    if (!filePath) {
      setUiState("error");
      setEnvelope(null);
      setLocalError("Unable to access local file path. Please run in Tauri desktop mode.");
      return;
    }

    void startParse(filePath);
  }

  function onFileChange(event: ChangeEvent<HTMLInputElement>) {
    if (event.target.files) {
      processFiles(event.target.files);
    }
    event.target.value = "";
  }

  function onDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setDragging(false);
    processFiles(event.dataTransfer.files);
  }

  function onDragOver(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setDragging(true);
  }

  function onDragLeave(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    setDragging(false);
  }

  function onRetry() {
    if (lastFilePath) {
      void startParse(lastFilePath);
    }
  }

  function onPickFile() {
    inputRef.current?.click();
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

  const data = envelope?.data;
  const renderErrors = uiState === "error" || (envelope && !envelope.success);
  const iconAvailable = Boolean(data?.iconUrl);

  const hasSignaturePartialRisk =
    envelope?.warnings.includes("SIGNATURE_PARTIAL") ||
    envelope?.warnings.includes("SIGNATURE_BLOCK_DETECTED_UNPARSED");

  return (
    <Container maxWidth="lg" sx={{ py: 4 }}>
      <Stack spacing={3}>
        <Box>
          <Typography variant="h4" fontWeight={700}>
            APK Info Quick View
          </Typography>
          <Typography variant="body1" color="text.secondary" mt={1}>
            Drag or select an APK file, then render the unified Envelope contract.
          </Typography>
        </Box>

        <Paper
          variant="outlined"
          sx={{
            p: 4,
            borderStyle: "dashed",
            borderWidth: 2,
            borderColor: dragging ? "primary.main" : "divider",
            backgroundColor: dragging ? "rgba(11, 87, 208, 0.05)" : "background.paper",
            transition: "all 0.2s ease"
          }}
          onDrop={onDrop}
          onDragOver={onDragOver}
          onDragLeave={onDragLeave}
        >
          <Stack spacing={2} alignItems="center">
            <CloudUploadIcon color="primary" sx={{ fontSize: 42 }} />
            <Typography variant="h6">Drop APK here</Typography>
            <Typography variant="body2" color="text.secondary">
              Only .apk is supported in v1.0
            </Typography>
            <Button variant="contained" onClick={onPickFile}>
              Select APK File
            </Button>
            <input
              ref={inputRef}
              type="file"
              accept=".apk,application/vnd.android.package-archive"
              style={{ display: "none" }}
              onChange={onFileChange}
            />
          </Stack>
        </Paper>

        {uiState === "loading" && (
          <Paper variant="outlined" sx={{ p: 3 }}>
            <Stack direction="row" spacing={2} alignItems="center">
              <CircularProgress size={24} />
              <Typography>Parsing APK, please wait...</Typography>
            </Stack>
          </Paper>
        )}

        {renderErrors && (
          <Paper variant="outlined" sx={{ p: 3 }}>
            <Stack spacing={2}>
              <Alert severity="error">
                {localError || envelope?.errorMessage || "Parsing failed. Please retry."}
              </Alert>
              {envelope?.errorCode && <Chip color="error" label={`Error Code: ${envelope.errorCode}`} />}
              <Stack direction="row" spacing={1}>
                <Button
                  variant="contained"
                  startIcon={<ReplayIcon />}
                  onClick={onRetry}
                  disabled={!canRetry || uiState === "loading"}
                >
                  Retry Parse
                </Button>
                <Button variant="outlined" onClick={onPickFile}>
                  Choose Another File
                </Button>
              </Stack>
            </Stack>
          </Paper>
        )}

        {data && (
          <Box
            sx={{
              display: "grid",
              gap: 2,
              gridTemplateColumns: { xs: "1fr", md: "repeat(12, 1fr)" }
            }}
          >
            <Box sx={{ gridColumn: { xs: "1 / -1", md: "span 8" } }}>
              <Paper variant="outlined" sx={{ p: 3 }}>
                <Typography variant="h6" gutterBottom>
                  Basic Info
                </Typography>
                <List dense>
                  <ListItem>
                    <ListItemText primary="Package Name" secondary={data.packageName || EMPTY_TEXT} />
                  </ListItem>
                  <Divider component="li" />
                  <ListItem>
                    <ListItemText primary="App Name" secondary={data.appName || EMPTY_TEXT} />
                  </ListItem>
                  <Divider component="li" />
                  <ListItem>
                    <ListItemText primary="Channel" secondary={data.channel || "unknown"} />
                  </ListItem>
                </List>
              </Paper>
            </Box>

            <Box sx={{ gridColumn: { xs: "1 / -1", md: "span 4" } }}>
              <Paper variant="outlined" sx={{ p: 3, height: "100%" }}>
                <Typography variant="h6" gutterBottom>
                  Icon
                </Typography>
                {iconAvailable ? (
                  <Stack spacing={2} alignItems="flex-start">
                    <Box
                      component="img"
                      src={data.iconUrl}
                      alt="app-icon"
                      sx={{
                        width: 96,
                        height: 96,
                        borderRadius: 2,
                        border: "1px solid",
                        borderColor: "divider"
                      }}
                    />
                    <Button
                      variant="outlined"
                      startIcon={<DownloadIcon />}
                      onClick={() => onDownloadIcon(data.iconUrl)}
                    >
                      Export Icon
                    </Button>
                  </Stack>
                ) : (
                  <Stack spacing={1}>
                    <Typography color="text.secondary">No icon resource found</Typography>
                    <Button variant="outlined" disabled startIcon={<DownloadIcon />}>
                      Export Icon
                    </Button>
                  </Stack>
                )}
              </Paper>
            </Box>

            <Box sx={{ gridColumn: "1 / -1" }}>
              <Paper variant="outlined" sx={{ p: 3 }}>
                <Typography variant="h6" gutterBottom>
                  Version Info
                </Typography>
                <List dense>
                  <ListItem>
                    <ListItemText primary="minSdkVersion" secondary={data.minSdkVersion} />
                  </ListItem>
                  <Divider component="li" />
                  <ListItem>
                    <ListItemText primary="targetSdkVersion" secondary={data.targetSdkVersion} />
                  </ListItem>
                  <Divider component="li" />
                  <ListItem>
                    <ListItemText
                      primary="compileSdkVersion"
                      secondary={data.compileSdkVersion ?? "null"}
                    />
                  </ListItem>
                  <Divider component="li" />
                  <ListItem>
                    <ListItemText primary="versionCode" secondary={data.versionCode} />
                  </ListItem>
                  <Divider component="li" />
                  <ListItem>
                    <ListItemText primary="versionName" secondary={data.versionName ?? "null"} />
                  </ListItem>
                </List>
              </Paper>
            </Box>

            <Box sx={{ gridColumn: { xs: "1 / -1", md: "span 6" } }}>
              <Paper variant="outlined" sx={{ p: 3, height: "100%" }}>
                <Typography variant="h6" gutterBottom>
                  Permissions
                </Typography>
                {data.permissions.length === 0 ? (
                  <Typography color="text.secondary">{EMPTY_TEXT}</Typography>
                ) : (
                  <List dense>
                    {data.permissions.map((permission) => (
                      <ListItem key={permission}>
                        <ListItemText primary={permission} />
                      </ListItem>
                    ))}
                  </List>
                )}
              </Paper>
            </Box>

            <Box sx={{ gridColumn: { xs: "1 / -1", md: "span 6" } }}>
              <Paper variant="outlined" sx={{ p: 3, height: "100%" }}>
                <Typography variant="h6" gutterBottom>
                  ABI List
                </Typography>
                {data.abis.length === 0 ? (
                  <Typography color="text.secondary">{EMPTY_TEXT}</Typography>
                ) : (
                  <Stack direction="row" spacing={1} useFlexGap flexWrap="wrap">
                    {data.abis.map((abi) => (
                      <Chip key={abi} label={abi} />
                    ))}
                  </Stack>
                )}
              </Paper>
            </Box>

            <Box sx={{ gridColumn: "1 / -1" }}>
              <Paper variant="outlined" sx={{ p: 3 }}>
                <Typography variant="h6" gutterBottom>
                  Signers
                </Typography>
                {hasSignaturePartialRisk && (
                  <Alert severity="warning" sx={{ mb: 2 }}>
                    Signature information is best-effort. Some certificate metadata may be incomplete.
                  </Alert>
                )}
                {data.signers.length === 0 ? (
                  <Typography color="text.secondary">{EMPTY_TEXT}</Typography>
                ) : (
                  <Stack spacing={2}>
                    {data.signers.map((signer, index) => (
                      <Paper key={`${signer.certSha256}-${index}`} variant="outlined" sx={{ p: 2 }}>
                        <Typography variant="subtitle2" gutterBottom>
                          Signer #{index + 1}
                        </Typography>
                        <List dense>
                          <ListItem>
                            <ListItemText primary="scheme" secondary={signer.scheme || EMPTY_TEXT} />
                          </ListItem>
                          <Divider component="li" />
                          <ListItem>
                            <ListItemText
                              primary="certSha256"
                              secondary={signer.certSha256 || EMPTY_TEXT}
                            />
                          </ListItem>
                          <Divider component="li" />
                          <ListItem>
                            <ListItemText primary="issuer" secondary={signer.issuer || EMPTY_TEXT} />
                          </ListItem>
                          <Divider component="li" />
                          <ListItem>
                            <ListItemText primary="subject" secondary={signer.subject || EMPTY_TEXT} />
                          </ListItem>
                          <Divider component="li" />
                          <ListItem>
                            <ListItemText
                              primary="validFrom"
                              secondary={formatDate(signer.validFrom)}
                            />
                          </ListItem>
                          <Divider component="li" />
                          <ListItem>
                            <ListItemText primary="validTo" secondary={formatDate(signer.validTo)} />
                          </ListItem>
                        </List>
                      </Paper>
                    ))}
                  </Stack>
                )}
              </Paper>
            </Box>

            <Box sx={{ gridColumn: "1 / -1" }}>
              <Paper variant="outlined" sx={{ p: 3 }}>
                <Typography variant="h6" gutterBottom>
                  Warnings
                </Typography>
                {envelope.warnings.length === 0 ? (
                  <Typography color="text.secondary">{EMPTY_TEXT}</Typography>
                ) : (
                  <Stack direction="row" spacing={1} useFlexGap flexWrap="wrap">
                    {envelope.warnings.map((warning) => (
                      <Chip key={warning} label={warning} color="warning" variant="outlined" />
                    ))}
                  </Stack>
                )}
              </Paper>
            </Box>
          </Box>
        )}
      </Stack>
    </Container>
  );
}

function formatDate(raw: string): string {
  if (!raw) {
    return EMPTY_TEXT;
  }
  const date = new Date(raw);
  if (Number.isNaN(date.getTime())) {
    return raw;
  }
  return date.toLocaleString();
}

export default App;
