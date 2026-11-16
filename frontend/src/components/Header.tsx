import { AppearanceControls } from "@/components/AppearanceControls";
import { useEffect, useState } from "react";
import { RefreshCw, LogOut, User } from "lucide-react";
import { toast } from "sonner";
import { useAuth } from "@/contexts/AuthContext";
import {
  healthCheck,
  type HealthResponse,
  updateSignatures,
} from "@/lib/api";
import logo from "../../logo.png";
import release from "@/version.json";

const Header = () => {
  const { auth, logout } = useAuth();
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [loadingHealth, setLoadingHealth] = useState(true);
  const [updatingSignatures, setUpdatingSignatures] = useState(false);

  useEffect(() => {
    let mounted = true;

    const loadHealth = async () => {
      try {
        const next = await healthCheck();
        if (mounted) setHealth(next);
      } catch {
        if (mounted) setHealth(null);
      } finally {
        if (mounted) setLoadingHealth(false);
      }
    };

    void loadHealth();
    const timer = window.setInterval(() => {
      void loadHealth();
    }, 30000);

    return () => {
      mounted = false;
      window.clearInterval(timer);
    };
  }, []);

  const handleUpdateSignatures = async () => {
    setUpdatingSignatures(true);
    try {
      const result = await updateSignatures(auth.token);
      toast.success("Signature update completed", {
        description: `freshclam exit code ${result.exitCode}`,
      });
      const next = await healthCheck();
      setHealth(next);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Signature update failed";
      toast.error("Signature update failed", {
        description: message,
      });
    } finally {
      setUpdatingSignatures(false);
    }
  };

  const engineReady = Boolean(health?.clamscan.ok && health?.freshclam.ok);
  const engineLabel = loadingHealth
    ? "Checking engine"
    : engineReady
    ? "Engine ready"
    : "Engine error";

  return (
    <header className="av-page-header border-b bg-card">
      <div className="av-header-content flex items-center justify-between">
        <div className="flex items-center gap-3">
          <img
            src={logo}
            alt="INFRA-AV"
            className="h-10 w-10 rounded-lg object-contain"
          />
          <div>
            <h1 className="av-page-title">Antivirus</h1>
            <p className="text-xs text-muted-foreground" title={`Backend ${health?.backendVersion ?? "connecting"}`}>{release.version}</p>
          </div>
        </div>
        <div className="av-header-actions flex items-center gap-3">
          <AppearanceControls />
          <div className="flex items-center gap-2 rounded-md bg-secondary px-3 py-1.5">
            <span
              className={`h-2 w-2 rounded-full ${
                engineReady ? "bg-success animate-pulse-dot" : "bg-destructive"
              }`}
            />
            <span className="text-xs font-medium text-secondary-foreground">
              {engineLabel}
            </span>
            {health && (
              <span className="text-xs text-muted-foreground">
                Q:{health.queue.pending}
              </span>
            )}
          </div>
          <button
            onClick={handleUpdateSignatures}
            disabled={updatingSignatures}
            className="flex items-center gap-1.5 rounded-md border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-secondary active:scale-[0.97] disabled:cursor-not-allowed disabled:opacity-50"
          >
            <RefreshCw
              className={`h-3.5 w-3.5 ${updatingSignatures ? "animate-spin" : ""}`}
            />
            {updatingSignatures ? "Updating..." : "Update signatures"}
          </button>
          <div className="h-5 w-px bg-border" />
          <div className="flex items-center gap-2 rounded-md bg-secondary px-3 py-1.5">
            <User className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-xs font-medium text-secondary-foreground">
              {auth.username}
            </span>
          </div>
          <button
            onClick={logout}
            className="flex items-center gap-1.5 rounded-md border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive active:scale-[0.97]"
          >
            <LogOut className="h-3.5 w-3.5" />
            Logout
          </button>
        </div>
      </div>
    </header>
  );
};

export default Header;
