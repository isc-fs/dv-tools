// Vite configuration for the ISC MingoROS frontend.
//
// Tauri 2 launches `vite` as a child process during `tauri dev` and expects the
// dev server on port 5173. For production builds Tauri sets TAURI_* env vars
// before invoking `npm run build`; the knobs below honour them so the bundle
// targets the right WebView. Mirrors MingoCAN's can-studio vite.config.ts.

import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
    plugins: [svelte()],
    // Prevent Vite from obscuring rust-side errors in the terminal.
    clearScreen: false,
    server: {
        port: 5173,
        strictPort: true,
        host: host !== undefined ? host : false,
        hmr:
            host !== undefined
                ? { protocol: 'ws', host, port: 5174 }
                : undefined,
        watch: { ignored: ['**/src-tauri/**'] },
    },
    // WebView targets: Edge WebView2 (Windows), WKWebView (macOS), WebKitGTK
    // (Linux). The intersection is ~ES2022; set explicitly so accidental
    // top-level-await regressions don't slip through.
    build: {
        target:
            process.env.TAURI_PLATFORM === 'windows'
                ? 'chrome105'
                : 'safari14',
        minify: process.env.TAURI_DEBUG === 'true' ? false : 'esbuild',
        sourcemap: process.env.TAURI_DEBUG === 'true',
        outDir: 'dist',
        emptyOutDir: true,
    },
}));
