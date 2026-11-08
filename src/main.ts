import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./styles/tokens.css";
import "./styles/theme.css";
import "./styles/palette.css";
import "./styles/typography.css";
import "./styles/animations.css";
createApp(App).use(createPinia()).mount("#app");
