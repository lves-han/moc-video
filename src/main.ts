import { createApp, h } from "vue";
import App from "./App.vue";
import UiProvider from "./ui/UiProvider.vue";
import "./style.css";

const View = location.hash === "#/prototype"
  ? (await import("./views/PrototypeView.vue")).default
  : App;
createApp({
  render: () => h(UiProvider, null, { default: () => h(View) }),
}).mount("#app");
