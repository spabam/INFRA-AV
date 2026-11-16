export {};
declare global {
  interface Window {
    VantageDesign?: {
      current(): { theme: "light" | "dark"; density: "comfortable" | "compact" };
      setTheme(theme: "light" | "dark"): void;
      setDensity(density: "comfortable" | "compact"): void;
      sync(): void;
    };
  }
}
