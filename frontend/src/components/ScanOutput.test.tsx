import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import ScanOutput from "./ScanOutput";
import type { ScanStatusResponse } from "@/lib/api";

afterEach(cleanup);
const status: ScanStatusResponse = {
  requestId: "fixture", status: "done", target: "/tmp/fixture", submittedBy: "owner",
  submittedAt: "2026-01-01T00:00:00Z", commandPreview: "fixture", output: [], reportAvailable: false,
};
function show(scanStatus: ScanStatusResponse) {
  render(<ScanOutput scanStatus={scanStatus} reportHtml="" reportFilename="" isScanning={false} isLoadingReport={false} onDownloadReport={() => {}} />);
}
describe("truthful scan outcomes", () => {
  it("does not call a completed job clean without a detection summary", () => {
    show(status);
    expect(screen.queryByText("Clean")).not.toBeInTheDocument();
    expect(screen.getByText("Summary unavailable")).toBeInTheDocument();
  });
  it("requires an explicit successful clean summary and measured zero", () => {
    show({ ...status, summary: { clean: true, infectedFiles: 0 } });
    expect(screen.getByText("Clean")).toBeInTheDocument();
  });
  it("retains a failed lifecycle even when findings were already produced", () => {
    show({ ...status, status: "failed", summary: { clean: false, infectedFiles: 1 } });
    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.queryByText("Clean")).not.toBeInTheDocument();
  });
  it("never calls a cancelled scan clean", () => {
    show({ ...status, status: "cancelled", summary: { clean: true, infectedFiles: 0 } });
    expect(screen.getByText("Stopped")).toBeInTheDocument();
    expect(screen.queryByText("Clean")).not.toBeInTheDocument();
  });
});
