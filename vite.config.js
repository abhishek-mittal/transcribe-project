import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: {
      // Proxy API calls to `vercel dev` during local development
      // Run `vercel dev` instead of `npm run dev` to get Python functions working locally
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true
      }
    }
  }
});
