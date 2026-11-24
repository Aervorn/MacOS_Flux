/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Le dossier "dist/" du dépôt est déjà utilisé par scripts/make-dmg.sh :
// le front compilé va donc dans "web-dist/".
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
  build: { outDir: "web-dist", emptyOutDir: true },
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/crates/**"] } },
});
