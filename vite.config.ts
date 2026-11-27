/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Le dossier "dist/" du dépôt est déjà utilisé par scripts/make-dmg.sh :
// le front compilé va donc dans "web-dist/".
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
  // L'app est lue depuis le Mac, pas téléchargée : la taille d'un fichier JavaScript n'a pas
  // d'importance, d'où une limite d'avertissement relevée.
  build: { outDir: "web-dist", emptyOutDir: true, chunkSizeWarningLimit: 4000 },
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/crates/**"] } },
});
