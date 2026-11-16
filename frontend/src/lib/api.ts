import type { ClamAVOptions } from "@/components/ScanOptions";

const API_BASE = import.meta.env.VITE_API_BASE || "";
const API_ENDPOINT = `${API_BASE}/api`;
export const AUTH_INVALID_EVENT = "infra-av-auth-invalid";

interface ApiError {
  message: string;
}

interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: ApiError;
}

export interface ScanAction {
  timestamp: string;
  user: string;
  type: "av";
  target: string;
  scanType: "file" | "directory" | "device";
  options: ClamAVOptions;
}

export interface LoginResponse {
  token: string;
  username: string;
  role: string;
}

export interface QueueCounts {
  pending: number;
  processing: number;
  done: number;
  failed: number;
}

export interface CommandProbe {
  ok: boolean;
  output: string;
}

export interface HealthResponse {
  productVersion?: string;
  backendVersion: string;
  clamscan: CommandProbe;
  freshclam: CommandProbe;
  queue: QueueCounts;
}

export interface ScanSummary {
  filesScanned?: number;
  infectedFiles?: number;
  directoriesScanned?: number;
  dataScanned?: string;
  dataRead?: string;
  duration?: string;
  startDate?: string;
  endDate?: string;
  clean: boolean;
}

export interface ScanSubmitResponse {
  requestId: string;
  filename: string;
  status: "pending" | "processing" | "paused" | "done" | "failed" | "cancelled";
  commandPreview: string;
}

export interface ScanStatusResponse {
  requestId: string;
  status: "pending" | "processing" | "paused" | "done" | "failed" | "cancelled";
  target: string;
  submittedBy: string;
  submittedAt: string;
  startedAt?: string;
  finishedAt?: string;
  commandPreview: string;
  output: string[];
  summary?: ScanSummary;
  errorMessage?: string;
  reportAvailable: boolean;
  reportFilename?: string;
}

export interface ScanReportResponse {
  requestId: string;
  filename: string;
  html: string;
}

export interface SignatureUpdateResponse {
  exitCode: number;
  output: string[];
}

export interface ScanBrowseEntry {
  name: string;
  path: string;
  kind: "root" | "directory" | "file" | "device";
  selectable: boolean;
  description?: string;
}

export interface ScanBrowseResponse {
  scanType: "file" | "directory" | "device";
  currentPath?: string;
  parentPath?: string;
  canSelectCurrent: boolean;
  roots: string[];
  entries: ScanBrowseEntry[];
}

export interface ScanControlResponse {
  requestId: string;
  status: "pending" | "processing" | "paused" | "done" | "failed" | "cancelled";
}

async function apiRequest<TRequest, TResponse>(
  action: string,
  payload: TRequest,
  token?: string
): Promise<TResponse> {
  const res = await fetch(API_ENDPOINT, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify({ action, payload }),
  });

  const data = (await res.json().catch(() => ({}))) as ApiResponse<TResponse>;
  if (!res.ok || !data.ok || !data.data) {
    const message = data.error?.message || "Request failed";
    if (token && isInvalidSessionResponse(res.status, message)) {
      window.dispatchEvent(
        new CustomEvent(AUTH_INVALID_EVENT, {
          detail: { message, action },
        })
      );
    }
    throw new Error(message);
  }

  return data.data;
}

function isInvalidSessionResponse(status: number, message: string): boolean {
  if (status === 401) return true;

  const normalized = message.toLowerCase();
  return (
    normalized.includes("invalid token") ||
    normalized.includes("missing authorization header") ||
    normalized.includes("authorization header must use bearer")
  );
}

export const loginRequest = (username: string, password: string) =>
  apiRequest<{ username: string; password: string }, LoginResponse>("auth.login", {
    username,
    password,
  });

// UUUUI single sign-on: exchange a broker handoff token for an INFRA-AV session.
export const ssoRequest = (brokerToken: string) =>
  apiRequest<{ token: string }, LoginResponse>("auth.sso", { token: brokerToken });

export const healthCheck = () =>
  apiRequest<Record<string, never>, HealthResponse>("health.check", {});

export const submitScanAction = (token: string, action: ScanAction) =>
  apiRequest<ScanAction, ScanSubmitResponse>("scan.submit", action, token);

export const getScanStatus = (token: string, requestId: string) =>
  apiRequest<{ requestId: string }, ScanStatusResponse>("scan.status", { requestId }, token);

export const getScanReport = (token: string, requestId: string) =>
  apiRequest<{ requestId: string }, ScanReportResponse>("scan.report", { requestId }, token);

export const runWorkerOnce = (token: string) =>
  apiRequest<Record<string, never>, { processed: boolean }>("scan.runWorkerOnce", {}, token);

export const browseTargets = (
  token: string,
  scanType: "file" | "directory" | "device",
  path?: string
) =>
  apiRequest<{ scanType: "file" | "directory" | "device"; path?: string }, ScanBrowseResponse>(
    "scan.browse",
    { scanType, ...(path ? { path } : {}) },
    token
  );

export const pauseScan = (token: string, requestId: string) =>
  apiRequest<{ requestId: string }, ScanControlResponse>("scan.pause", { requestId }, token);

export const resumeScan = (token: string, requestId: string) =>
  apiRequest<{ requestId: string }, ScanControlResponse>("scan.resume", { requestId }, token);

export const stopScan = (token: string, requestId: string) =>
  apiRequest<{ requestId: string }, ScanControlResponse>("scan.stop", { requestId }, token);

export const updateSignatures = (token: string) =>
  apiRequest<Record<string, never>, SignatureUpdateResponse>("signatures.update", {}, token);

export const validateSession = (token: string) =>
  apiRequest<{ limit: number }, { items: Array<unknown> }>("scan.list", { limit: 1 }, token);
