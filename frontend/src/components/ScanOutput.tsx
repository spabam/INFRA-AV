import { Download, FileText, LoaderCircle, ShieldAlert, ShieldCheck } from "lucide-react";

import type { ScanStatusResponse } from "@/lib/api";

interface ScanOutputProps {
  scanStatus: ScanStatusResponse | null;
  reportHtml: string;
  reportFilename: string;
  isScanning: boolean;
  isLoadingReport: boolean;
  onDownloadReport: () => void;
}

const buildTimeline = (scanStatus: ScanStatusResponse) => [
  ["Request ID", scanStatus.requestId],
  ["Submitted By", scanStatus.submittedBy],
  ["Target", scanStatus.target],
  ["Queued At", new Date(scanStatus.submittedAt).toLocaleString()],
  ...(scanStatus.startedAt
    ? [["Started At", new Date(scanStatus.startedAt).toLocaleString()]]
    : []),
  ...(scanStatus.finishedAt
    ? [["Finished At", new Date(scanStatus.finishedAt).toLocaleString()]]
    : []),
];

const ScanOutput = ({
  scanStatus,
  reportHtml,
  reportFilename,
  isScanning,
  isLoadingReport,
  onDownloadReport,
}: ScanOutputProps) => {
  if (!scanStatus) {
    return (
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <FileText className="h-4 w-4 text-muted-foreground" />
          <label className="text-sm font-medium">HTML Scan Report</label>
        </div>
        <div className="rounded-xl border border-dashed bg-foreground/[0.03] p-8 text-center">
          <p className="text-sm text-muted-foreground">
            Submit a scan to generate the INFRA-AV HTML report.
          </p>
        </div>
      </div>
    );
  }

  const infectedFiles = scanStatus.summary?.infectedFiles || 0;
  const clean = scanStatus.status === "done" && infectedFiles === 0;
  const statusLabel =
    scanStatus.status === "pending"
      ? "Queued"
      : scanStatus.status === "processing"
      ? "Running"
      : scanStatus.status === "paused"
      ? "Paused"
      : scanStatus.status === "cancelled"
      ? "Stopped"
      : clean
      ? "Clean"
      : infectedFiles > 0
      ? "Threats Found"
      : "Completed With Errors";
  const statusTone =
    scanStatus.status === "pending" || scanStatus.status === "processing"
      ? "text-primary bg-primary/10"
      : scanStatus.status === "paused" || scanStatus.status === "cancelled"
      ? "text-warning bg-warning/10"
      : clean
      ? "text-success bg-success/10"
      : "text-destructive bg-destructive/10";

  const timeline = buildTimeline(scanStatus);
  const canDownload = Boolean(reportHtml && reportFilename);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <FileText className="h-4 w-4 text-muted-foreground" />
          <label className="text-sm font-medium">HTML Scan Report</label>
          {isScanning && (
            <div className="flex items-center gap-1">
              <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-primary" />
              <span
                className="h-1.5 w-1.5 animate-pulse rounded-full bg-primary"
                style={{ animationDelay: "0.2s" }}
              />
              <span
                className="h-1.5 w-1.5 animate-pulse rounded-full bg-primary"
                style={{ animationDelay: "0.4s" }}
              />
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          <span className={`rounded-full px-3 py-1 text-xs font-semibold ${statusTone}`}>
            {statusLabel}
          </span>
          <button
            onClick={onDownloadReport}
            disabled={!canDownload}
            className="inline-flex items-center gap-2 rounded-lg border bg-card px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-secondary disabled:cursor-not-allowed disabled:opacity-50"
          >
            <Download className="h-3.5 w-3.5" />
            Download HTML
          </button>
        </div>
      </div>

      <div className="grid gap-3 rounded-xl border bg-foreground/[0.02] p-4 md:grid-cols-2 xl:grid-cols-4">
        {timeline.map(([label, value]) => (
          <div key={label} className="rounded-lg border bg-background px-3 py-2">
            <div className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
              {label}
            </div>
            <div className="mt-1 break-all font-mono text-xs text-foreground/80">
              {value}
            </div>
          </div>
        ))}
      </div>

      {(scanStatus.status === "pending" ||
        scanStatus.status === "processing" ||
        scanStatus.status === "paused") && (
        <div className="flex items-start gap-3 rounded-xl border bg-primary/5 p-5">
          <LoaderCircle className="mt-0.5 h-5 w-5 animate-spin text-primary" />
          <div className="space-y-1">
            <p className="text-sm font-semibold text-foreground">
              {scanStatus.status === "pending"
                ? "The request is queued for the worker."
                : scanStatus.status === "paused"
                ? "INFRA-AV paused the scan and can resume from the current point."
                : "INFRA-AV is executing the scan now."}
            </p>
            <p className="text-sm text-muted-foreground">
              {scanStatus.status === "paused"
                ? "Use Resume to continue or Stop to terminate the running clamscan process."
                : "The HTML report will appear here as soon as the scan reaches a terminal state."}
            </p>
          </div>
        </div>
      )}

      {scanStatus.status !== "pending" &&
        scanStatus.status !== "processing" &&
        scanStatus.status !== "paused" && (
        <>
          {isLoadingReport && (
            <div className="flex items-center gap-3 rounded-xl border bg-secondary/40 p-4 text-sm text-muted-foreground">
              <LoaderCircle className="h-4 w-4 animate-spin" />
              Preparing the HTML report...
            </div>
          )}

          {!isLoadingReport && reportHtml && (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                {clean ? (
                  <ShieldCheck className="h-4 w-4 text-success" />
                ) : (
                  <ShieldAlert className="h-4 w-4 text-destructive" />
                )}
                <span>
                  Report file: <span className="font-mono">{reportFilename}</span>
                </span>
              </div>
              <iframe
                title="INFRA-AV Scan Report"
                sandbox=""
                srcDoc={reportHtml}
                className="h-[720px] w-full rounded-xl border bg-white"
              />
            </div>
          )}

          {!isLoadingReport && !reportHtml && (
            <div className="rounded-xl border border-dashed bg-secondary/20 p-6 text-sm text-muted-foreground">
              The scan completed, but the HTML report is not available yet.
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default ScanOutput;
