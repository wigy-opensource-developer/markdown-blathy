import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port and serves the built frontend from `dist`.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
