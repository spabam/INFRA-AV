import { Copy, Check } from "lucide-react";
import { useState } from "react";
import { ClamAVOptions, defaultOptions } from "./ScanOptions";

interface CommandPreviewProps {
  target: string;
  scanType: "file" | "directory" | "device";
  options: ClamAVOptions;
}

const CommandPreview = ({ target, scanType, options }: CommandPreviewProps) => {
  const [copied, setCopied] = useState(false);

  const buildCommand = () => {
    const args: string[] = ["clamscan"];

    if (options.recursive && scanType === "directory") args.push("--recursive");
    if (options.infected) args.push("--infected");
    if (options.remove) args.push("--remove");
    if (options.move) args.push(`--move=${options.move}`);
    if (options.copy) args.push(`--copy=${options.copy}`);
    if (options.log) args.push(`--log=${options.log}`);

    if (options.excludeDir) args.push(`--exclude-dir="${options.excludeDir}"`);
    if (options.exclude) args.push(`--exclude="${options.exclude}"`);
    if (options.includeDir) args.push(`--include-dir="${options.includeDir}"`);
    if (options.include) args.push(`--include="${options.include}"`);

    if (options.maxFileSize !== defaultOptions.maxFileSize) args.push(`--max-filesize=${options.maxFileSize}`);
    if (options.maxScanSize !== defaultOptions.maxScanSize) args.push(`--max-scansize=${options.maxScanSize}`);
    if (options.maxRecursion !== defaultOptions.maxRecursion) args.push(`--max-recursion=${options.maxRecursion}`);
    if (options.maxFiles !== defaultOptions.maxFiles) args.push(`--max-files=${options.maxFiles}`);

    if (!options.crossFs) args.push("--cross-fs=no");
    args.push(`--follow-dir-symlinks=${options.followDirSymlinks ? 1 : 0}`);
    args.push(`--follow-file-symlinks=${options.followFileSymlinks ? 1 : 0}`);

    if (!options.scanPe) args.push("--scan-pe=no");
    if (!options.scanElf) args.push("--scan-elf=no");
    if (!options.scanOle2) args.push("--scan-ole2=no");
    if (!options.scanPdf) args.push("--scan-pdf=no");
    if (!options.scanSwf) args.push("--scan-swf=no");
    if (!options.scanHwp3) args.push("--scan-hwp3=no");
    if (!options.scanXmlDocs) args.push("--scan-xmldocs=no");
    if (!options.scanMail) args.push("--scan-mail=no");
    if (!options.scanHtml) args.push("--scan-html=no");
    if (!options.scanArchive) args.push("--scan-archive=no");

    if (!options.heuristicAlerts) args.push("--heuristic-alerts=no");
    if (options.detectPua) {
      args.push("--detect-pua=yes");
      if (options.excludePua) args.push(`--exclude-pua="${options.excludePua}"`);
      if (options.includePua) args.push(`--include-pua="${options.includePua}"`);
    }
    if (options.detectStructured) {
      args.push("--detect-structured=yes");
      if (options.structuredSsnCount !== defaultOptions.structuredSsnCount) args.push(`--structured-ssn-count=${options.structuredSsnCount}`);
      if (options.structuredCcCount !== defaultOptions.structuredCcCount) args.push(`--structured-cc-count=${options.structuredCcCount}`);
    }

    if (options.alertBrokenExecutables) args.push("--alert-broken=yes");
    if (options.alertEncrypted) args.push("--alert-encrypted=yes");
    if (options.alertEncryptedArchive) args.push("--alert-encrypted-archive=yes");
    if (options.alertEncryptedDoc) args.push("--alert-encrypted-doc=yes");
    if (options.alertMacros) args.push("--alert-macros=yes");
    if (options.alertPhishingSSLMismatch) args.push("--alert-phishing-ssl=yes");
    if (options.alertPhishingCloak) args.push("--alert-phishing-cloak=yes");
    if (options.alertPartitionIntersection) args.push("--alert-partition-intersection=yes");

    if (options.database) args.push(`--database=${options.database}`);
    if (options.officialDbOnly) args.push("--official-db-only=yes");
    if (!options.bytecode) args.push("--bytecode=no");
    else if (options.bytecodeTimeout !== defaultOptions.bytecodeTimeout) args.push(`--bytecode-timeout=${options.bytecodeTimeout}`);

    if (options.verbose) args.push("--verbose");
    if (options.debug) args.push("--debug");
    if (options.quiet) args.push("--quiet");
    if (options.bell) args.push("--bell");
    if (options.noSummary) args.push("--no-summary");
    if (options.stdout) args.push("--stdout");

    if (!options.normalize) args.push("--normalize=no");
    if (options.tempdir) args.push(`--tempdir=${options.tempdir}`);
    if (options.leaveTemps) args.push("--leave-temps=yes");

    args.push(target || "/path/to/scan");

    return args.join(" \\\n  ");
  };

  const command = buildCommand();

  const handleCopy = () => {
    navigator.clipboard.writeText(command.split(" \\\n  ").join(" "));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <label className="text-sm font-medium">Generated Command</label>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground active:scale-[0.95]"
        >
          {copied ? (
            <>
              <Check className="h-3.5 w-3.5 text-success" />
              Copied
            </>
          ) : (
            <>
              <Copy className="h-3.5 w-3.5" />
              Copy
            </>
          )}
        </button>
      </div>
      <pre className="overflow-x-auto rounded-lg border bg-foreground/[0.03] p-4 font-mono text-xs leading-relaxed text-foreground/80">
        {command}
      </pre>
    </div>
  );
};

export default CommandPreview;
