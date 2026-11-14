import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
export default defineConfig({
  define: { __APP_VERSION__: JSON.stringify(process.env.npm_package_version || "0.1.0-beta7") },
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
