import { defineConfig, type Plugin } from "vite";
import { cpSync } from "node:fs";
import { execFileSync } from "node:child_process";
import react from "@vitejs/plugin-react-swc";
import path from "path";
import { componentTagger } from "lovable-tagger";
import { designRuntime } from "./design-runtime";


// Every standalone or gateway build ships the complete verified design release,
// including font licenses. This uses only the local snapshot, never UUUUI paths.
function vantageDesign(): Plugin {
  let output = "";
  const source = path.resolve(__dirname, "src/vantage-design");
  return {
    name: "vantage-design-release",
    apply: "build",
    configResolved(config) { output = path.resolve(config.root, config.build.outDir); },
    buildStart() { execFileSync("python3", [path.join(source, "verify.py"), "--check"], { stdio: "inherit" }); },
    closeBundle() { cpSync(source, path.join(output, "vantage-design"), { recursive: true }); },
  };
}

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  base: process.env.VITE_BASE || "/",
  server: {
    host: "::",
    port: 8080,
    hmr: {
      overlay: false,
    },
  },
  plugins: [designRuntime(), vantageDesign(), react(), mode === "development" && componentTagger()].filter(Boolean),
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
}));
