import { AppController } from "./state.ts";
import { previewApi } from "./data/preview.ts";
import { preferenceQueue } from "./preferences.ts";

export const app = new AppController(previewApi, preferenceQueue, () => {
  const video = typeof document === "undefined" ? null : document.querySelector("video");
  if (video) { video.pause(); video.removeAttribute("src"); video.load(); }
});
