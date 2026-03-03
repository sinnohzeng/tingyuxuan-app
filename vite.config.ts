import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { sentryVitePlugin } from "@sentry/vite-plugin";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [
    react(),
    tailwindcss(),
    // Sentry source maps 上传（仅在 CI release 构建时生效）
    ...(process.env.SENTRY_AUTH_TOKEN
      ? [
          sentryVitePlugin({
            org: process.env.SENTRY_ORG,
            project: process.env.SENTRY_PROJECT,
            authToken: process.env.SENTRY_AUTH_TOKEN,
            url: process.env.SENTRY_URL,
            sourcemaps: {
              filesToDeleteAfterUpload: ["**/*.js.map"],
            },
          }),
        ]
      : []),
  ],
  build: {
    sourcemap: true,
  },
  // 排除 Rust target 目录，避免 Vite 扫描 target/doc/ 下的 HTML 文件
  exclude: ["**/target/**"],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
