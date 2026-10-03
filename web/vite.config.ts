import { defineConfig } from "vite";

// In production the app is served under /webrace/ (Caddy subdirectory). Builds
// with `vite build --base=/webrace/` set base automatically; here we only
// override it when the WB_BASE env var is set, so `npm run dev` stays at root.
const base = process.env.WB_BASE ?? "/";

export default defineConfig({
  base,
  build: {
    target: "es2022",
    sourcemap: true,
  },
  server: {
    // WebTransport requires secure context; localhost counts as secure.
    host: "127.0.0.1",
    proxy: {
      "/maps": {
        target: "http://127.0.0.1:4173",
        changeOrigin: true,
      },
      "/tex": {
        target: "http://127.0.0.1:4173",
        changeOrigin: true,
      },
      "/catalog": {
        target: "http://127.0.0.1:4173",
        changeOrigin: true,
      },
      "/snd": {
        target: "http://127.0.0.1:4173",
        changeOrigin: true,
      },
      "/sounds": {
        target: "http://127.0.0.1:4173",
        changeOrigin: true,
      },
      "/api": {
        target: "http://127.0.0.1:4174",
        changeOrigin: true,
      },
    },
  },
  optimizeDeps: {
    exclude: [],
  },
});
