// Apply saved appearance before CSS and React paint without inline scripts.
// The normal distribution plugin copies the same verified runtime into dist.
import { readFile } from "node:fs/promises";
import path from "node:path";
import type { Plugin } from "vite";

export function designRuntime(): Plugin {
  const source = path.resolve(__dirname, "src/vantage-design/runtime.js");
  let assetUrl = "/vantage-design/runtime.js";
  let assetPath = assetUrl;
  return {
    name: "vantage-appearance-before-paint",
    configResolved(config) {
      const base = config.base.endsWith("/") ? config.base : `${config.base}/`;
      assetUrl = `${base}vantage-design/runtime.js`;
      assetPath = new URL(assetUrl, "http://vantage.invalid/").pathname;
    },
    transformIndexHtml: {
      order: "post",
      handler() {
        return [{ tag: "script", attrs: { src: assetUrl }, injectTo: "head-prepend" }];
      },
    },
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        const pathname = request.url?.split("?")[0];
        // Vite may already have stripped its configured base from this request.
        if (pathname !== assetPath && pathname !== "/vantage-design/runtime.js") { next(); return; }
        try {
          response.setHeader("Content-Type", "text/javascript; charset=utf-8");
          response.setHeader("Cache-Control", "no-cache");
          response.end(await readFile(source));
        } catch (error) { next(error); }
      });
    },
  };
}
