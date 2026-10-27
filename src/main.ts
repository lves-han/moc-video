import { createApp, h } from "vue";
import App from "./App.vue";
import UiProvider from "./ui/UiProvider.vue";
import "./style.css";
import { restorePreferences } from "./preferences";
import { app } from "./app";

// Settings is restored before the first component frame (including native startup).
try { app.state.settings = await restorePreferences(); }
catch (error) { app.diagnostics.push({ code: "INTERNAL_ERROR", message: `Settings: ${error instanceof Error ? error.message : String(error)}`, retryable: true }); }
await app.refreshRuntime();
await app.loadSources();

const View = location.hash === "#/prototype"
  ? (await import("./views/PrototypeView.vue")).default
  : App;
createApp({
  render: () => h(UiProvider, null, { default: () => h(View) }),
}).mount("#app");
