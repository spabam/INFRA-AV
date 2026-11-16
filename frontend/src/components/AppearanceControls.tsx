import { useEffect, useState } from "react";

type Appearance = { theme: "light" | "dark"; density: "comfortable" | "compact" };
type DesignRuntime = { current: () => Appearance; setTheme: (theme: Appearance["theme"]) => void; setDensity: (density: Appearance["density"]) => void };
const runtime = () => (window as Window & { VantageDesign?: DesignRuntime }).VantageDesign;

/** Standalone controls; the platform owns appearance when embedded. */
export function AppearanceControls() {
  const [appearance, setAppearance] = useState<Appearance>(() => runtime()?.current() ?? { theme: "light", density: "comfortable" });
  useEffect(() => {
    const update = () => { const next = runtime()?.current(); if (next) setAppearance(next); };
    update();
    window.addEventListener("vantage-appearance", update);
    return () => window.removeEventListener("vantage-appearance", update);
  }, []);
  if (window.self !== window.top) return null;
  return <div className="vantage-appearance" aria-label="Appearance">
    <button type="button" aria-pressed={appearance.theme === "dark"} onClick={() => runtime()?.setTheme(appearance.theme === "dark" ? "light" : "dark")}>Dark theme</button>
    <button type="button" aria-pressed={appearance.density === "compact"} onClick={() => runtime()?.setDensity(appearance.density === "compact" ? "comfortable" : "compact")}>Compact density</button>
  </div>;
}
