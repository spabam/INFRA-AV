import { useState, useCallback, useEffect } from "react";
import { Pause, Play, RotateCcw, Square } from "lucide-react";
import { toast } from "sonner";
import Header from "@/components/Header";
import ScanTarget from "@/components/ScanTarget";
import ScanOptions, { ClamAVOptions, defaultOptions } from "@/components/ScanOptions";
import CommandPreview from "@/components/CommandPreview";
import ScanOutput from "@/components/ScanOutput";
import { useAuth } from "@/contexts/AuthContext";
import {
  getScanReport,
  getScanStatus,
  pauseScan,
  resumeScan,
  runWorkerOnce,
  submitScanAction,
  stopScan,
  type ScanAction,
  type ScanStatusResponse,
} from "@/lib/api";

const getErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : "Request failed";

const isActiveScan = (status?: ScanStatusResponse["status"]) =>
  status === "pending" || status === "processing" || status === "paused";

const needsFollowUpPoll = (status: ScanStatusResponse | null) =>
  status &&
  (isActiveScan(status.status) ||
    (status.status === "cancelled" && !status.finishedAt && !status.reportAvailable));

const Index = () => {
  const { auth } = useAuth();
  const [target, setTarget] = useState("");
  const [scanType, setScanType] = useState<"file" | "directory" | "device">("directory");
  const [options, setOptions] = useState<ClamAVOptions>(defaultOptions);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [activeTab, setActiveTab] = useState<"options" | "command">("options");
  const [currentRequestId, setCurrentRequestId] = useState("");
  const [scanStatus, setScanStatus] = useState<ScanStatusResponse | null>(null);
  const [storedReportHtml, setReportHtml] = useState("");
  const [storedReportFilename, setReportFilename] = useState("");
  const [isLoadingReport, setIsLoadingReport] = useState(false);
  const reportHtml = scanStatus?.reportAvailable ? storedReportHtml : "";
  const reportFilename = scanStatus?.reportAvailable ? storedReportFilename : "";

  useEffect(() => {
    if (!currentRequestId) return;

    let cancelled = false;
    let timeoutId: number | undefined;

    const pollStatus = async () => {
      try {
        const next = await getScanStatus(auth.token, currentRequestId);
        if (cancelled) return;

        setScanStatus(next);

        if (needsFollowUpPoll(next)) {
          timeoutId = window.setTimeout(() => {
            void pollStatus();
          }, 3000);
        }
      } catch (error) {
        if (cancelled) return;
        toast.error("Failed to load scan status", {
          description: getErrorMessage(error),
        });
      }
    };

    void pollStatus();

    return () => {
      cancelled = true;
      if (timeoutId) window.clearTimeout(timeoutId);
    };
  }, [auth.token, currentRequestId]);

  useEffect(() => {
    if (
      !scanStatus ||
      (scanStatus.status !== "done" &&
        scanStatus.status !== "failed" &&
        scanStatus.status !== "cancelled")
    ) {
      return;
    }

    if (!scanStatus.reportAvailable) {
      return;
    }

    let cancelled = false;

    const loadReport = async () => {
      setIsLoadingReport(true);
      try {
        const report = await getScanReport(auth.token, scanStatus.requestId);
        if (cancelled) return;
        setReportHtml(report.html);
        setReportFilename(report.filename);
      } catch (error) {
        if (cancelled) return;
        setReportHtml("");
        setReportFilename(scanStatus.reportFilename || "");
        toast.error("Failed to load HTML report", {
          description: getErrorMessage(error),
        });
      } finally {
        if (!cancelled) setIsLoadingReport(false);
      }
    };

    void loadReport();

    return () => {
      cancelled = true;
    };
  }, [auth.token, scanStatus]);

  const isScanRunning = isActiveScan(scanStatus?.status);
  const canPause = scanStatus?.status === "processing";
  const canResume = scanStatus?.status === "paused";
  const canStop = scanStatus?.status === "pending" || scanStatus?.status === "processing" || scanStatus?.status === "paused";

  const handleDownloadReport = useCallback(() => {
    if (!reportHtml || !reportFilename) return;

    const blob = new Blob([reportHtml], { type: "text/html;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = reportFilename;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }, [reportFilename, reportHtml]);

  const refreshStatus = useCallback(async () => {
    if (!currentRequestId) return;
    const next = await getScanStatus(auth.token, currentRequestId);
    setScanStatus(next);
  }, [auth.token, currentRequestId]);

  const handleStartScan = useCallback(async () => {
    if (!target) return;

    setIsSubmitting(true);
    setScanStatus(null);
    setCurrentRequestId("");
    setReportHtml("");
    setReportFilename("");
    setIsLoadingReport(false);

    const now = new Date();

    const action: ScanAction = {
      timestamp: now.toISOString(),
      user: auth.username,
      type: "av",
      target,
      scanType,
      options,
    };

    try {
      const result = await submitScanAction(auth.token, action);
      setCurrentRequestId(result.requestId);
      toast.success("Scan action queued", {
        description: result.filename,
      });

      void runWorkerOnce(auth.token).catch(() => undefined);
    } catch (error) {
      toast.error("Failed to submit scan action", {
        description: getErrorMessage(error),
      });
    } finally {
      setIsSubmitting(false);
    }
  }, [target, scanType, options, auth]);

  const handlePauseScan = useCallback(async () => {
    if (!currentRequestId) return;
    try {
      const result = await pauseScan(auth.token, currentRequestId);
      toast.success("Scan paused", {
        description: `Request ${result.requestId} is now paused.`,
      });
      await refreshStatus();
    } catch (error) {
      toast.error("Failed to pause scan", {
        description: getErrorMessage(error),
      });
    }
  }, [auth.token, currentRequestId, refreshStatus]);

  const handleResumeScan = useCallback(async () => {
    if (!currentRequestId) return;
    try {
      const result = await resumeScan(auth.token, currentRequestId);
      toast.success("Scan resumed", {
        description: `Request ${result.requestId} is running again.`,
      });
      await refreshStatus();
    } catch (error) {
      toast.error("Failed to resume scan", {
        description: getErrorMessage(error),
      });
    }
  }, [auth.token, currentRequestId, refreshStatus]);

  const handleStopScan = useCallback(async () => {
    if (!currentRequestId) return;
    try {
      const result = await stopScan(auth.token, currentRequestId);
      toast.success("Scan stop requested", {
        description: `Request ${result.requestId} is stopping.`,
      });
      await refreshStatus();
    } catch (error) {
      toast.error("Failed to stop scan", {
        description: getErrorMessage(error),
      });
    }
  }, [auth.token, currentRequestId, refreshStatus]);

  const resetAll = () => {
    setTarget("");
    setOptions(defaultOptions);
    setIsSubmitting(false);
    setCurrentRequestId("");
    setScanStatus(null);
    setReportHtml("");
    setReportFilename("");
    setIsLoadingReport(false);
  };

  const statCards = scanStatus?.summary
    ? [
        {
          label: "Files scanned",
          value: scanStatus.summary.filesScanned?.toString() || "-",
          color: "text-foreground",
        },
        {
          label: "Threats found",
          value: scanStatus.summary.infectedFiles?.toString() ?? "Unknown",
          color:
            (scanStatus.summary.infectedFiles || 0) > 0
              ? "text-destructive"
              : scanStatus.status === "done" && scanStatus.summary.clean && scanStatus.summary.infectedFiles === 0
                ? "text-success"
                : "text-muted-foreground",
        },
        {
          label: "Scan time",
          value: scanStatus.summary.duration || "-",
          color: "text-foreground",
        },
      ]
    : [];

  return (
    <div className="av-workspace min-h-screen bg-background">
      <Header />

      <main className="av-page-content">
        <div className="av-workspace-grid grid gap-6">
          {/* Left Panel: Config */}
          <div className="animate-fade-up space-y-4">
            <div className="rounded-lg border bg-card p-5 shadow-sm">
              <ScanTarget
                token={auth.token}
                target={target}
                onTargetChange={setTarget}
                scanType={scanType}
                onScanTypeChange={setScanType}
              />
            </div>

            <div className="rounded-lg border bg-card shadow-sm">
              <div className="flex items-center border-b">
                <button
                  onClick={() => setActiveTab("options")}
                  className={`flex-1 px-4 py-3 text-sm font-medium transition-colors ${
                    activeTab === "options"
                      ? "border-b-2 border-primary text-primary"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  Options
                </button>
                <button
                  onClick={() => setActiveTab("command")}
                  className={`flex-1 px-4 py-3 text-sm font-medium transition-colors ${
                    activeTab === "command"
                      ? "border-b-2 border-primary text-primary"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  Command
                </button>
              </div>

              <div className="max-h-[calc(100vh-380px)] overflow-y-auto p-4">
                {activeTab === "options" ? (
                  <ScanOptions options={options} onChange={setOptions} />
                ) : (
                  <CommandPreview target={target} scanType={scanType} options={options} />
                )}
              </div>
            </div>
          </div>

          {/* Right Panel: Actions & Output */}
          <div className="animate-fade-up-delay-1 space-y-4">
            <div className="flex flex-wrap gap-3">
              <button
                onClick={handleStartScan}
                disabled={!target || isSubmitting || isScanRunning}
                className="flex flex-1 items-center justify-center gap-2 rounded-lg bg-primary px-6 py-3 text-sm font-semibold text-primary-foreground shadow-sm transition-all hover:bg-primary/90 disabled:opacity-40 disabled:cursor-not-allowed active:scale-[0.98]"
              >
                {isSubmitting || isScanRunning ? (
                  <>
                    <Square className="h-4 w-4 animate-spin" />
                    {isSubmitting ? "Submitting..." : "Running..."}
                  </>
                ) : (
                  <>
                    <Play className="h-4 w-4" />
                    Start scan
                  </>
                )}
              </button>
              <button
                onClick={handlePauseScan}
                disabled={!canPause}
                className="flex items-center gap-2 rounded-lg border bg-card px-4 py-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 active:scale-[0.97]"
              >
                <Pause className="h-4 w-4" />
                Pause
              </button>
              <button
                onClick={handleResumeScan}
                disabled={!canResume}
                className="flex items-center gap-2 rounded-lg border bg-card px-4 py-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 active:scale-[0.97]"
              >
                <Play className="h-4 w-4" />
                Resume
              </button>
              <button
                onClick={handleStopScan}
                disabled={!canStop}
                className="flex items-center gap-2 rounded-lg border bg-card px-4 py-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:cursor-not-allowed disabled:opacity-40 active:scale-[0.97]"
              >
                <Square className="h-4 w-4" />
                Stop
              </button>
              <button
                onClick={resetAll}
                className="flex items-center gap-2 rounded-lg border bg-card px-4 py-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground active:scale-[0.97]"
              >
                <RotateCcw className="h-4 w-4" />
                Reset
              </button>
            </div>

            <div className="rounded-lg border bg-card p-5 shadow-sm">
              <ScanOutput
                scanStatus={scanStatus}
                reportHtml={reportHtml}
                reportFilename={reportFilename}
                isScanning={isSubmitting || isScanRunning}
                isLoadingReport={isLoadingReport}
                onDownloadReport={handleDownloadReport}
              />
            </div>

            {statCards.length > 0 && !isSubmitting && (
              <div className="animate-fade-up grid grid-cols-3 gap-3">
                {statCards.map((stat) => (
                  <div key={stat.label} className="rounded-lg border bg-card p-4 text-center shadow-sm">
                    <div className={`text-2xl font-bold tabular-nums ${stat.color}`}>{stat.value}</div>
                    <div className="mt-1 text-xs text-muted-foreground">{stat.label}</div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </main>
    </div>
  );
};

export default Index;
