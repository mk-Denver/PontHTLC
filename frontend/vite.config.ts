import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

const apiTarget = process.env.PONTHTLC_API ?? "http://127.0.0.1:9800";

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: apiTarget, changeOrigin: true, ws: true },
      "/health": { target: apiTarget, changeOrigin: true },
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
