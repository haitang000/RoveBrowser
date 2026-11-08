import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
export default defineConfig({
  plugins: [vue()],
  base: "./",
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: [
        "**/native/**",
        "**/.reference/**",
        "**/.local/**",
        "**/licenses/**",
        "**/test-results/**",
      ],
    },
  },
});
