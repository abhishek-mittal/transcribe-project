import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// Local dev: `npm run dev:all` runs Vite + the local Python API server together.
// Vite proxies /api/* to http://127.0.0.1:8787 (the dev_api.py wrapper).
//
// Production on the Vultr VPS: Nginx reverse-proxies /api/* to Gunicorn on :8000
// and / to the SvelteKit Node server on :3000 (see deploy/nginx.conf).
const API_PORT = process.env.DEV_API_PORT || '8787';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: {
      '/api': {
        target: `http://127.0.0.1:${API_PORT}`,
        changeOrigin: true,
      },
    },
  },
});
