import { useState } from "react";
import { ChevronDown } from "lucide-react";

export interface ClamAVOptions {
  recursive: boolean;
  infected: boolean;
  remove: boolean;
  move: string;
  copy: string;
  excludeDir: string;
  exclude: string;
  includeDir: string;
  include: string;
  maxFileSize: string;
  maxScanSize: string;
  maxRecursion: string;
  maxFiles: string;
  crossFs: boolean;
  followDirSymlinks: boolean;
  followFileSymlinks: boolean;
  bell: boolean;
  noSummary: boolean;
  stdout: boolean;
  verbose: boolean;
  debug: boolean;
  quiet: boolean;
  log: string;
  database: string;
  officialDbOnly: boolean;
  bytecode: boolean;
  bytecodeTimeout: string;
  detectPua: boolean;
  excludePua: string;
  includePua: string;
  detectStructured: boolean;
  structuredSsnCount: string;
  structuredCcCount: string;
  scanPe: boolean;
  scanElf: boolean;
  scanOle2: boolean;
  scanPdf: boolean;
  scanSwf: boolean;
  scanHwp3: boolean;
  scanXmlDocs: boolean;
  scanMail: boolean;
  scanHtml: boolean;
  scanArchive: boolean;
  alertBrokenExecutables: boolean;
  alertEncrypted: boolean;
  alertEncryptedArchive: boolean;
  alertEncryptedDoc: boolean;
  alertMacros: boolean;
  alertPhishingSSLMismatch: boolean;
  alertPhishingCloak: boolean;
  alertPartitionIntersection: boolean;
  heuristicAlerts: boolean;
  normalize: boolean;
  tempdir: string;
  leaveTemps: boolean;
}

export const defaultOptions: ClamAVOptions = {
  recursive: true,
  infected: false,
  remove: false,
  move: "",
  copy: "",
  excludeDir: "",
  exclude: "",
  includeDir: "",
  include: "",
  maxFileSize: "100M",
  maxScanSize: "400M",
  maxRecursion: "17",
  maxFiles: "10000",
  crossFs: true,
  followDirSymlinks: false,
  followFileSymlinks: false,
  bell: false,
  noSummary: false,
  stdout: false,
  verbose: false,
  debug: false,
  quiet: false,
  log: "",
  database: "",
  officialDbOnly: false,
  bytecode: true,
  bytecodeTimeout: "10000",
  detectPua: false,
  excludePua: "",
  includePua: "",
  detectStructured: false,
  structuredSsnCount: "3",
  structuredCcCount: "3",
  scanPe: true,
  scanElf: true,
  scanOle2: true,
  scanPdf: true,
  scanSwf: true,
  scanHwp3: true,
  scanXmlDocs: true,
  scanMail: true,
  scanHtml: true,
  scanArchive: true,
  alertBrokenExecutables: false,
  alertEncrypted: false,
  alertEncryptedArchive: false,
  alertEncryptedDoc: false,
  alertMacros: false,
  alertPhishingSSLMismatch: false,
  alertPhishingCloak: false,
  alertPartitionIntersection: false,
  heuristicAlerts: true,
  normalize: true,
  tempdir: "",
  leaveTemps: false,
};

interface ScanOptionsProps {
  options: ClamAVOptions;
  onChange: (options: ClamAVOptions) => void;
}

const Toggle = ({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) => (
  <label className="flex cursor-pointer items-start gap-3 rounded-md p-2 transition-colors hover:bg-secondary/60">
    <button
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`mt-0.5 h-5 w-9 flex-shrink-0 rounded-full transition-colors active:scale-[0.95] ${
        checked ? "bg-primary" : "bg-border"
      }`}
    >
      <span
        className={`block h-4 w-4 rounded-full bg-card shadow-sm transition-transform ${
          checked ? "translate-x-[18px]" : "translate-x-0.5"
        }`}
      />
    </button>
    <div>
      <span className="text-sm font-medium">{label}</span>
      {description && (
        <p className="text-xs text-muted-foreground">{description}</p>
      )}
    </div>
  </label>
);

const TextInput = ({
  label,
  value,
  onChange,
  placeholder,
  mono,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  mono?: boolean;
}) => (
  <div className="space-y-1 p-2">
    <label className="text-sm font-medium">{label}</label>
    <input
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className={`w-full rounded-md border bg-card px-3 py-2 text-sm placeholder:text-muted-foreground/60 focus:outline-none focus:ring-2 focus:ring-ring ${
        mono ? "font-mono" : ""
      }`}
    />
  </div>
);

const Section = ({
  title,
  children,
  defaultOpen = false,
}: {
  title: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
}) => {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border-b last:border-b-0">
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between px-2 py-3 text-sm font-semibold transition-colors hover:text-primary active:scale-[0.99]"
      >
        {title}
        <ChevronDown
          className={`h-4 w-4 text-muted-foreground transition-transform ${
            open ? "rotate-180" : ""
          }`}
        />
      </button>
      {open && <div className="pb-3">{children}</div>}
    </div>
  );
};

const ScanOptions = ({ options, onChange }: ScanOptionsProps) => {
  const set = <K extends keyof ClamAVOptions>(key: K, value: ClamAVOptions[K]) =>
    onChange({ ...options, [key]: value });

  return (
    <div className="space-y-0">
      <Section title="General Options" defaultOpen>
        <Toggle label="Recursive" description="Scan directories recursively" checked={options.recursive} onChange={(v) => set("recursive", v)} />
        <Toggle label="Infected Only" description="Only print infected files" checked={options.infected} onChange={(v) => set("infected", v)} />
        <Toggle label="Remove Infected" description="Remove infected files (USE WITH CAUTION)" checked={options.remove} onChange={(v) => set("remove", v)} />
        <TextInput label="Move to" value={options.move} onChange={(v) => set("move", v)} placeholder="/quarantine/path" mono />
        <TextInput label="Copy to" value={options.copy} onChange={(v) => set("copy", v)} placeholder="/backup/path" mono />
        <TextInput label="Log File" value={options.log} onChange={(v) => set("log", v)} placeholder="/var/log/clamscan.log" mono />
      </Section>

      <Section title="Filters & Exclusions">
        <TextInput label="Exclude Directories (regex)" value={options.excludeDir} onChange={(v) => set("excludeDir", v)} placeholder="^/proc|^/sys" mono />
        <TextInput label="Exclude Files (regex)" value={options.exclude} onChange={(v) => set("exclude", v)} placeholder="\\.log$" mono />
        <TextInput label="Include Directories (regex)" value={options.includeDir} onChange={(v) => set("includeDir", v)} placeholder="^/home" mono />
        <TextInput label="Include Files (regex)" value={options.include} onChange={(v) => set("include", v)} placeholder="\\.exe$|\\.dll$" mono />
      </Section>

      <Section title="Size & Depth Limits">
        <TextInput label="Max File Size" value={options.maxFileSize} onChange={(v) => set("maxFileSize", v)} placeholder="100M" />
        <TextInput label="Max Scan Size" value={options.maxScanSize} onChange={(v) => set("maxScanSize", v)} placeholder="400M" />
        <TextInput label="Max Recursion Depth" value={options.maxRecursion} onChange={(v) => set("maxRecursion", v)} placeholder="17" />
        <TextInput label="Max Files in Archive" value={options.maxFiles} onChange={(v) => set("maxFiles", v)} placeholder="10000" />
      </Section>

      <Section title="Symlinks & Filesystems">
        <Toggle label="Cross Filesystems" description="Scan files on other filesystems" checked={options.crossFs} onChange={(v) => set("crossFs", v)} />
        <Toggle label="Follow Directory Symlinks" checked={options.followDirSymlinks} onChange={(v) => set("followDirSymlinks", v)} />
        <Toggle label="Follow File Symlinks" checked={options.followFileSymlinks} onChange={(v) => set("followFileSymlinks", v)} />
      </Section>

      <Section title="File Type Scanning">
        <Toggle label="PE (Windows Executables)" checked={options.scanPe} onChange={(v) => set("scanPe", v)} />
        <Toggle label="ELF (Linux Executables)" checked={options.scanElf} onChange={(v) => set("scanElf", v)} />
        <Toggle label="OLE2 (MS Office)" checked={options.scanOle2} onChange={(v) => set("scanOle2", v)} />
        <Toggle label="PDF Documents" checked={options.scanPdf} onChange={(v) => set("scanPdf", v)} />
        <Toggle label="SWF (Flash)" checked={options.scanSwf} onChange={(v) => set("scanSwf", v)} />
        <Toggle label="HWP3 Documents" checked={options.scanHwp3} onChange={(v) => set("scanHwp3", v)} />
        <Toggle label="XML Documents" checked={options.scanXmlDocs} onChange={(v) => set("scanXmlDocs", v)} />
        <Toggle label="Mail Files" checked={options.scanMail} onChange={(v) => set("scanMail", v)} />
        <Toggle label="HTML Files" checked={options.scanHtml} onChange={(v) => set("scanHtml", v)} />
        <Toggle label="Archives" checked={options.scanArchive} onChange={(v) => set("scanArchive", v)} />
      </Section>

      <Section title="Alerts & Heuristics">
        <Toggle label="Heuristic Alerts" description="Enable heuristic-based detection" checked={options.heuristicAlerts} onChange={(v) => set("heuristicAlerts", v)} />
        <Toggle label="Detect PUA" description="Potentially Unwanted Applications" checked={options.detectPua} onChange={(v) => set("detectPua", v)} />
        {options.detectPua && (
          <>
            <TextInput label="Exclude PUA Categories" value={options.excludePua} onChange={(v) => set("excludePua", v)} placeholder="NetTool,PWTool" />
            <TextInput label="Include PUA Categories" value={options.includePua} onChange={(v) => set("includePua", v)} placeholder="Spy,Packed" />
          </>
        )}
        <Toggle label="Detect Structured Data" description="SSN and credit card numbers" checked={options.detectStructured} onChange={(v) => set("detectStructured", v)} />
        {options.detectStructured && (
          <>
            <TextInput label="Min SSN Count" value={options.structuredSsnCount} onChange={(v) => set("structuredSsnCount", v)} placeholder="3" />
            <TextInput label="Min CC Count" value={options.structuredCcCount} onChange={(v) => set("structuredCcCount", v)} placeholder="3" />
          </>
        )}
        <Toggle label="Alert: Broken Executables" checked={options.alertBrokenExecutables} onChange={(v) => set("alertBrokenExecutables", v)} />
        <Toggle label="Alert: Any Encrypted Content" checked={options.alertEncrypted} onChange={(v) => set("alertEncrypted", v)} />
        <Toggle label="Alert: Encrypted Archives" checked={options.alertEncryptedArchive} onChange={(v) => set("alertEncryptedArchive", v)} />
        <Toggle label="Alert: Encrypted Documents" checked={options.alertEncryptedDoc} onChange={(v) => set("alertEncryptedDoc", v)} />
        <Toggle label="Alert: OLE2 Macros" checked={options.alertMacros} onChange={(v) => set("alertMacros", v)} />
        <Toggle label="Alert: SSL Mismatch (Phishing)" checked={options.alertPhishingSSLMismatch} onChange={(v) => set("alertPhishingSSLMismatch", v)} />
        <Toggle label="Alert: Phishing Cloak" checked={options.alertPhishingCloak} onChange={(v) => set("alertPhishingCloak", v)} />
        <Toggle label="Alert: Partition Intersection" checked={options.alertPartitionIntersection} onChange={(v) => set("alertPartitionIntersection", v)} />
      </Section>

      <Section title="Database & Bytecode">
        <TextInput label="Custom Database Path" value={options.database} onChange={(v) => set("database", v)} placeholder="/var/lib/clamav" mono />
        <Toggle label="Official DB Only" description="Only load official signatures" checked={options.officialDbOnly} onChange={(v) => set("officialDbOnly", v)} />
        <Toggle label="Bytecode" description="Load bytecode from database" checked={options.bytecode} onChange={(v) => set("bytecode", v)} />
        {options.bytecode && (
          <TextInput label="Bytecode Timeout (ms)" value={options.bytecodeTimeout} onChange={(v) => set("bytecodeTimeout", v)} placeholder="10000" />
        )}
      </Section>

      <Section title="Output & Verbosity">
        <Toggle label="Verbose" checked={options.verbose} onChange={(v) => set("verbose", v)} />
        <Toggle label="Debug" description="Enable debug messages" checked={options.debug} onChange={(v) => set("debug", v)} />
        <Toggle label="Quiet" description="Suppress all output except errors" checked={options.quiet} onChange={(v) => set("quiet", v)} />
        <Toggle label="Bell" description="Sound bell on detection" checked={options.bell} onChange={(v) => set("bell", v)} />
        <Toggle label="No Summary" description="Suppress scan summary" checked={options.noSummary} onChange={(v) => set("noSummary", v)} />
        <Toggle label="Use stdout" description="Write output to stdout instead of stderr" checked={options.stdout} onChange={(v) => set("stdout", v)} />
      </Section>

      <Section title="Advanced">
        <Toggle label="Normalize" description="Normalize HTML, scripts, and text files" checked={options.normalize} onChange={(v) => set("normalize", v)} />
        <TextInput label="Temp Directory" value={options.tempdir} onChange={(v) => set("tempdir", v)} placeholder="/tmp" mono />
        <Toggle label="Leave Temps" description="Do not remove temporary files" checked={options.leaveTemps} onChange={(v) => set("leaveTemps", v)} />
      </Section>
    </div>
  );
};

export default ScanOptions;
