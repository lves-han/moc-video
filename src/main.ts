import { createApp, h } from "vue";
import App from "./App.vue";
import UiProvider from "./ui/UiProvider.vue";
import "./style.css";

createApp({
  render: () => h(UiProvider, null, { default: () => h(App) }),
}).mount("#app");
