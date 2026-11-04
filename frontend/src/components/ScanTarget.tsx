import { useEffect, useMemo, useState } from "react";
import {
  ArrowUp,
  ChevronRight,
  File,
  Folder,
  FolderOpen,
  HardDrive,
  Loader2,
  RefreshCw,
  X,
} from "lucide-react";

import {
  browseTargets,
  type ScanBrowseEntry,
  type ScanBrowseResponse,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface ScanTargetProps {
  token: string;
  target: string;
  onTargetChange: (target: string) => void;
  scanType: "file" | "directory" | "device";
  onScanTypeChange: (type: "file" | "directory" | "device") => void;
}

type FilesystemScanType = "file" | "directory";

const scanTypes = [
  { id: "file" as const, icon: File, label: "File" },
  { id: "directory" as const, icon: Folder, label: "Folder" },
  { id: "device" as const, icon: HardDrive, label: "Device" },
];

const isFilesystemScanType = (
  scanType: "file" | "directory" | "device"
): scanType is FilesystemScanType => scanType !== "device";

const getEntryIcon = (kind: ScanBrowseEntry["kind"]) => {
  switch (kind) {
    case "file":
      return File;
    case "device":
      return HardDrive;
    default:
      return Folder;
  }
};

const getLocationLabel = (path: string) => {
  if (path === "/") return "/";
  const segments = path.split("/").filter(Boolean);
  return segments[segments.length - 1] || path;
};

const resolveMatchingRoot = (currentPath: string | undefined, roots: string[]) => {
  if (!currentPath || roots.length === 0) return undefined;

  return (
    [...roots]
      .sort((left, right) => right.length - left.length)
      .find((root) => currentPath === root || currentPath.startsWith(`${root}/`)) || roots[0]
  );
};

const buildBreadcrumbs = (currentPath: string | undefined, roots: string[]) => {
  if (!currentPath) return [];

  const matchingRoot = resolveMatchingRoot(currentPath, roots) || currentPath;
  const items = [{ label: matchingRoot, path: matchingRoot }];
  const suffix = currentPath.slice(matchingRoot.length).split("/").filter(Boolean);
  let cursor = matchingRoot;

  for (const part of suffix) {
    cursor = cursor.endsWith("/") ? `${cursor}${part}` : `${cursor}/${part}`;
    items.push({ label: part, path: cursor });
  }

  return items;
};

const getParentDirectory = (path: string) => {
  if (!path || path === "/") return "/";

  const normalized = path.endsWith("/") && path !== "/" ? path.slice(0, -1) : path;
  const lastSlash = normalized.lastIndexOf("/");
  if (lastSlash <= 0) return "/";

  return normalized.slice(0, lastSlash);
};

const filesystemTitle = (scanType: FilesystemScanType) =>
  scanType === "file" ? "Choose File" : "Choose Folder";

const filesystemButtonLabel = (scanType: FilesystemScanType) =>
  scanType === "file" ? "Browse Filesystem" : "Browse Folders";

const filesystemPlaceholder = (scanType: FilesystemScanType) =>
  scanType === "file" ? "No file selected" : "No folder selected";

const filesystemSelectionLabel = (scanType: FilesystemScanType) =>
  scanType === "file" ? "Select File" : "Select Folder";

const ScanTarget = ({
  token,
  target,
  onTargetChange,
  scanType,
  onScanTypeChange,
}: ScanTargetProps) => {
  const [browserPath, setBrowserPath] = useState<string | undefined>(undefined);
  const [browser, setBrowser] = useState<ScanBrowseResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [reloadNonce, setReloadNonce] = useState(0);
  const [focusedPath, setFocusedPath] = useState("");
  const [isPickerOpen, setIsPickerOpen] = useState(false);

  useEffect(() => {
    const shouldLoad = scanType === "device" || isPickerOpen;
    if (!shouldLoad) return;

    let cancelled = false;

    const loadBrowser = async () => {
      setLoading(true);
      setError("");

      try {
        const response = await browseTargets(
          token,
          scanType,
          scanType === "device" ? undefined : browserPath
        );
        if (cancelled) return;
        setBrowser(response);
      } catch (err) {
        if (cancelled) return;
        setBrowser(null);
        setError(err instanceof Error ? err.message : "Failed to load available targets");
      } finally {
        if (!cancelled) setLoading(false);
      }
    };

    void loadBrowser();

    return () => {
      cancelled = true;
    };
  }, [token, scanType, browserPath, reloadNonce, isPickerOpen]);

  const filesystemScanType = isFilesystemScanType(scanType) ? scanType : undefined;
  const breadcrumbs = buildBreadcrumbs(browser?.currentPath, browser?.roots || []);
  const activeRoot = resolveMatchingRoot(browser?.currentPath, browser?.roots || []);

  const selectedEntry = useMemo(
    () => browser?.entries.find((entry) => entry.path === focusedPath),
    [browser?.entries, focusedPath]
  );

  const canConfirmFilesystemSelection = useMemo(() => {
    if (!filesystemScanType || !browser) return false;

    if (
      filesystemScanType === "directory" &&
      browser.canSelectCurrent &&
      browser.currentPath &&
      focusedPath === browser.currentPath
    ) {
      return true;
    }

    return Boolean(selectedEntry?.selectable);
  }, [browser, filesystemScanType, focusedPath, selectedEntry]);

  const handleTypeChange = (type: "file" | "directory" | "device") => {
    onScanTypeChange(type);
    onTargetChange("");
    setFocusedPath("");
    setBrowser(null);
    setBrowserPath(undefined);
    setError("");
    setIsPickerOpen(false);
  };

  const handleOpenPath = (path: string) => {
    setBrowserPath(path);
    setFocusedPath("");
  };

  const openFilesystemPicker = () => {
    if (!filesystemScanType) return;

    setError("");
    if (target) {
      setFocusedPath(target);
      setBrowserPath(
        filesystemScanType === "file" ? getParentDirectory(target) : target
      );
    }
    setIsPickerOpen(true);
  };

  const handleSelectEntry = (entry: ScanBrowseEntry) => {
    setFocusedPath(entry.path);
  };

  const handleActivateEntry = (entry: ScanBrowseEntry) => {
    if (entry.kind === "directory" || entry.kind === "root") {
      handleOpenPath(entry.path);
      return;
    }

    if (entry.selectable) {
      onTargetChange(entry.path);
      setFocusedPath(entry.path);
      setIsPickerOpen(false);
    }
  };

  const handleConfirmFilesystemSelection = () => {
    if (!filesystemScanType || !browser) return;

    if (
      filesystemScanType === "directory" &&
      browser.canSelectCurrent &&
      browser.currentPath &&
      focusedPath === browser.currentPath
    ) {
      onTargetChange(browser.currentPath);
      setIsPickerOpen(false);
      return;
    }

    if (selectedEntry?.selectable) {
      onTargetChange(selectedEntry.path);
      setIsPickerOpen(false);
    }
  };

  const handleSelectCurrentFolder = () => {
    if (filesystemScanType !== "directory" || !browser?.currentPath) return;

    setFocusedPath(browser.currentPath);
    onTargetChange(browser.currentPath);
    setIsPickerOpen(false);
  };

  const renderFilesystemChooser = () => {
    if (!filesystemScanType) return null;

    return (
      <>
        <div className="rounded-xl border bg-card p-4">
          <div className="space-y-3">
            <label className="text-sm font-medium">Scan Target</label>
            <div className="flex flex-col gap-3 md:flex-row">
              <Button onClick={openFilesystemPicker} className="md:w-auto">
                <FolderOpen className="h-4 w-4" />
                {filesystemButtonLabel(filesystemScanType)}
              </Button>
              <div className="min-w-0 flex-1 rounded-lg border bg-secondary/20 px-3 py-2">
                <div className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  Selected Path
                </div>
                <div className="mt-1 truncate font-mono text-sm text-foreground">
                  {target || filesystemPlaceholder(filesystemScanType)}
                </div>
              </div>
              {target && (
                <Button variant="outline" onClick={() => onTargetChange("")}>
                  <X className="h-4 w-4" />
                  Clear
                </Button>
              )}
            </div>
            <p className="text-xs text-muted-foreground">
              Open the chooser, move through one directory at a time, and select a single{" "}
              {filesystemScanType === "file" ? "file" : "folder"}.
            </p>
          </div>
        </div>

        <Dialog open={isPickerOpen} onOpenChange={setIsPickerOpen}>
          <DialogContent className="max-w-5xl gap-0 overflow-hidden p-0">
            <DialogHeader className="border-b px-6 py-4">
              <DialogTitle>{filesystemTitle(filesystemScanType)}</DialogTitle>
              <DialogDescription>
                Navigate the allowed filesystem hierarchically and choose a single{" "}
                {filesystemScanType === "file" ? "file" : "folder"}.
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4 px-6 py-4">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                <div className="flex flex-1 flex-col gap-3 sm:flex-row sm:items-center">
                  {browser?.roots && browser.roots.length > 0 && activeRoot && (
                    <Select value={activeRoot} onValueChange={handleOpenPath}>
                      <SelectTrigger className="w-full sm:w-[220px]">
                        <SelectValue placeholder="Choose location" />
                      </SelectTrigger>
                      <SelectContent>
                        {browser.roots.map((root) => (
                          <SelectItem key={root} value={root}>
                            {root}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}

                  <div className="flex min-w-0 flex-1 items-center gap-2 rounded-lg border bg-background px-3 py-2">
                    {browser?.parentPath && (
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => handleOpenPath(browser.parentPath!)}
                        aria-label="Go up"
                      >
                        <ArrowUp className="h-4 w-4" />
                      </Button>
                    )}
                    <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
                      {breadcrumbs.map((crumb, index) => (
                        <div key={crumb.path} className="flex min-w-0 items-center">
                          {index > 0 && (
                            <ChevronRight className="mx-1 h-3.5 w-3.5 text-muted-foreground" />
                          )}
                          <button
                            type="button"
                            onClick={() => handleOpenPath(crumb.path)}
                            className="truncate text-sm text-foreground transition-colors hover:text-primary"
                          >
                            {index === 0 ? getLocationLabel(crumb.label) : crumb.label}
                          </button>
                        </div>
                      ))}
                      {!breadcrumbs.length && !loading && (
                        <span className="text-sm text-muted-foreground">Loading location...</span>
                      )}
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  {filesystemScanType === "directory" && browser?.currentPath && (
                    <Button type="button" onClick={handleSelectCurrentFolder}>
                      Select Current Folder
                    </Button>
                  )}
                  <Button
                    type="button"
                    variant="outline"
                    size="icon"
                    onClick={() => setReloadNonce((value) => value + 1)}
                    aria-label="Refresh"
                  >
                    <RefreshCw className="h-4 w-4" />
                  </Button>
                </div>
              </div>

              <div className="overflow-hidden rounded-xl border bg-card">
                <div className="grid grid-cols-[minmax(0,1fr)_220px] border-b bg-secondary/20 px-4 py-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  <div>Name</div>
                  <div>Details</div>
                </div>

                <div className="max-h-[420px] overflow-y-auto">
                  {loading && (
                    <div className="flex items-center gap-2 px-4 py-6 text-sm text-muted-foreground">
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Loading current directory...
                    </div>
                  )}

                  {!loading && error && (
                    <div className="px-4 py-6 text-sm text-destructive">{error}</div>
                  )}

                  {!loading && !error && browser && browser.entries.length === 0 && (
                    <div className="px-4 py-6 text-sm text-muted-foreground">
                      This directory is empty.
                    </div>
                  )}

                  {!loading &&
                    !error &&
                    browser?.entries.map((entry) => {
                      const Icon = getEntryIcon(entry.kind);
                      const active = focusedPath === entry.path;
                      const disabledInFolderMode =
                        filesystemScanType === "directory" &&
                        entry.kind === "file" &&
                        !entry.selectable;

                      return (
                        <button
                          key={`${entry.kind}:${entry.path}`}
                          type="button"
                          onClick={() => handleSelectEntry(entry)}
                          onDoubleClick={() => handleActivateEntry(entry)}
                          className={`grid w-full grid-cols-[minmax(0,1fr)_220px] items-center gap-4 border-b px-4 py-3 text-left transition-colors last:border-b-0 ${
                            active
                              ? "bg-primary/10 ring-1 ring-inset ring-primary/20"
                              : "hover:bg-secondary/40"
                          } ${disabledInFolderMode ? "opacity-75" : ""}`}
                        >
                          <div className="flex min-w-0 items-center gap-3">
                            <div className="rounded-md bg-secondary p-2 text-muted-foreground">
                              <Icon className="h-4 w-4" />
                            </div>
                            <div className="min-w-0">
                              <div className="truncate text-sm font-medium text-foreground">
                                {entry.name}
                              </div>
                              <div className="truncate text-xs text-muted-foreground">
                                {entry.path}
                              </div>
                            </div>
                          </div>
                          <div className="truncate text-xs text-muted-foreground">
                            {entry.kind === "directory"
                              ? "Double click to open"
                              : disabledInFolderMode
                                ? "Visible, but files cannot be selected in folder mode"
                                : entry.description || "Selectable"}
                          </div>
                        </button>
                      );
                    })}
                </div>
              </div>

              <div className="rounded-lg border bg-secondary/10 px-4 py-3">
                <div className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  Pending Selection
                </div>
                <div className="mt-1 truncate font-mono text-sm text-foreground">
                  {focusedPath || target || filesystemPlaceholder(filesystemScanType)}
                </div>
              </div>
            </div>

            <DialogFooter className="border-t px-6 py-4">
              <Button type="button" variant="outline" onClick={() => setIsPickerOpen(false)}>
                Cancel
              </Button>
              <Button
                type="button"
                onClick={handleConfirmFilesystemSelection}
                disabled={!canConfirmFilesystemSelection}
              >
                {filesystemSelectionLabel(filesystemScanType)}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </>
    );
  };

  const renderDeviceChooser = () => (
    <div className="overflow-hidden rounded-xl border bg-card">
      <div className="flex items-center justify-between gap-3 border-b px-4 py-3">
        <div>
          <label className="text-sm font-medium">Scan Target</label>
          <p className="mt-1 text-xs text-muted-foreground">
            Select a single block device from the detected list.
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="icon"
          onClick={() => setReloadNonce((value) => value + 1)}
          aria-label="Refresh devices"
        >
          <RefreshCw className="h-4 w-4" />
        </Button>
      </div>

      <div className="grid grid-cols-[minmax(0,1fr)_220px] border-b bg-secondary/20 px-4 py-2 text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        <div>Name</div>
        <div>Details</div>
      </div>

      <div className="max-h-[320px] overflow-y-auto">
        {loading && (
          <div className="flex items-center gap-2 px-4 py-6 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            Detecting block devices...
          </div>
        )}

        {!loading && error && (
          <div className="px-4 py-6 text-sm text-destructive">{error}</div>
        )}

        {!loading && !error && browser && browser.entries.length === 0 && (
          <div className="px-4 py-6 text-sm text-muted-foreground">
            No block devices are available.
          </div>
        )}

        {!loading &&
          !error &&
          browser?.entries.map((entry) => {
            const Icon = getEntryIcon(entry.kind);
            const active = target === entry.path;

            return (
              <button
                key={`${entry.kind}:${entry.path}`}
                type="button"
                onClick={() => onTargetChange(entry.path)}
                className={`grid w-full grid-cols-[minmax(0,1fr)_220px] items-center gap-4 border-b px-4 py-3 text-left transition-colors last:border-b-0 ${
                  active ? "bg-primary/10 ring-1 ring-inset ring-primary/20" : "hover:bg-secondary/40"
                }`}
              >
                <div className="flex min-w-0 items-center gap-3">
                  <div className="rounded-md bg-secondary p-2 text-muted-foreground">
                    <Icon className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium text-foreground">
                      {entry.name}
                    </div>
                    <div className="truncate text-xs text-muted-foreground">
                      {entry.path}
                    </div>
                  </div>
                </div>
                <div className="truncate text-xs text-muted-foreground">
                  {entry.description || "Selectable"}
                </div>
              </button>
            );
          })}
      </div>

      <div className="border-t bg-secondary/10 px-4 py-3">
        <div className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
          Selected
        </div>
        <div className="mt-1 truncate font-mono text-sm text-foreground">
          {target || "No device selected"}
        </div>
      </div>
    </div>
  );

  return (
    <div className="space-y-4">
      <div className="space-y-3">
        <label className="text-sm font-medium">Target Type</label>
        <div className="flex gap-2">
          {scanTypes.map(({ id, icon: Icon, label }) => (
            <button
              key={id}
              type="button"
              onClick={() => handleTypeChange(id)}
              className={`flex items-center gap-2 rounded-md border px-3 py-2 text-sm transition-all active:scale-[0.97] ${
                scanType === id
                  ? "border-primary bg-accent text-accent-foreground shadow-sm"
                  : "bg-card text-muted-foreground hover:bg-secondary"
              }`}
            >
              <Icon className="h-4 w-4" />
              {label}
            </button>
          ))}
        </div>
      </div>

      {filesystemScanType ? renderFilesystemChooser() : renderDeviceChooser()}
    </div>
  );
};

export default ScanTarget;
