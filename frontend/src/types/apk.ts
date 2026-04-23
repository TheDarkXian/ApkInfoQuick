export interface Signer {
  scheme: string;
  certSha256: string;
  issuer: string;
  subject: string;
  validFrom: string;
  validTo: string;
}

export interface ApkInfoData {
  packageName: string;
  appName: string;
  iconUrl: string;
  minSdkVersion: number;
  targetSdkVersion: number;
  compileSdkVersion: number | null;
  versionCode: number;
  versionName: string | null;
  permissions: string[];
  signers: Signer[];
  abis: string[];
  channel: string;
}

export interface ApkInfoEnvelope {
  success: boolean;
  data: ApkInfoData;
  errorCode: string | null;
  errorMessage: string | null;
  warnings: string[];
}

const defaultData: ApkInfoData = {
  packageName: "unknown",
  appName: "Unknown",
  iconUrl: "",
  minSdkVersion: 1,
  targetSdkVersion: 1,
  compileSdkVersion: null,
  versionCode: 1,
  versionName: null,
  permissions: [],
  signers: [],
  abis: [],
  channel: "unknown"
};

function asObject(value: unknown): Record<string, unknown> {
  if (typeof value === "object" && value !== null) {
    return value as Record<string, unknown>;
  }
  return {};
}

function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function asNullableString(value: unknown): string | null {
  if (typeof value === "string") {
    return value;
  }
  return null;
}

function asNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function asNullableNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === "string");
}

function normalizeSigners(value: unknown): Signer[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.map((item): Signer => {
    const signer = asObject(item);
    return {
      scheme: asString(signer.scheme),
      certSha256: asString(signer.certSha256),
      issuer: asString(signer.issuer),
      subject: asString(signer.subject),
      validFrom: asString(signer.validFrom),
      validTo: asString(signer.validTo)
    };
  });
}

export function normalizeEnvelope(input: unknown): ApkInfoEnvelope {
  const raw = asObject(input);
  const rawData = asObject(raw.data);

  return {
    success: raw.success === true,
    data: {
      packageName: asString(rawData.packageName, defaultData.packageName),
      appName: asString(rawData.appName, defaultData.appName),
      iconUrl: asString(rawData.iconUrl),
      minSdkVersion: asNumber(rawData.minSdkVersion, defaultData.minSdkVersion),
      targetSdkVersion: asNumber(rawData.targetSdkVersion, defaultData.targetSdkVersion),
      compileSdkVersion: asNullableNumber(rawData.compileSdkVersion),
      versionCode: asNumber(rawData.versionCode, defaultData.versionCode),
      versionName: asNullableString(rawData.versionName),
      permissions: asStringArray(rawData.permissions),
      signers: normalizeSigners(rawData.signers),
      abis: asStringArray(rawData.abis),
      channel: asString(rawData.channel, defaultData.channel)
    },
    errorCode: asNullableString(raw.errorCode),
    errorMessage: asNullableString(raw.errorMessage),
    warnings: asStringArray(raw.warnings)
  };
}
