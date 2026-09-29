import { defineConfig } from "vite";

export default defineConfig({
  build: {
    target: "es2022",
    sourcemap: true,
  },
  server: {
    // WebTransport requires secure context; localhost counts as secure.
    host: "127.0.0.1",
  },
  optimizeDeps: {
    exclude: [],
  },
});
