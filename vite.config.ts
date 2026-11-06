import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue({ template: { compilerOptions: { isCustomElement: tag => ["video-player", "video-skin", "hlsjs-video", "media-i18n"].includes(tag) } } })],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022" },
});
