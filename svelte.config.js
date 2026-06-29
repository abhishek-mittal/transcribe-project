// SvelteKit configuration for the transcribe-project.
//
// Primary build target: Tauri desktop app (static, SPA-fallback).
//   - Uses @sveltejs/adapter-static so the output is a static bundle
//     loadable by Tauri's system webview with no Node server.
//   - Set `prerender = false` and `ssr = false` so the app behaves like
//     a single-page app (matches the previous Vite/SvelteKit dev UX).
//
// Reference / future hosted path: Node SSR (dormant).
//   - To re-enable, swap the import below to adapter from
//     '@sveltejs/adapter-node' and remove the fallback option.
//   - See deploy/ for the Vultr + Nginx deployment artifacts that
//     consume the Node build.
//
// Both paths are kept in the repo intentionally so we can ship the
// desktop app now and switch back to a hosted offering later without
// losing configuration.

import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    // SPA mode for Tauri: every route falls back to index.html so the
    // SvelteKit router can handle navigation client-side.
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: 'index.html',
      precompress: false,
      strict: true
    }),
    // Disable SSR — Tauri loads the bundle as static HTML/JS/CSS.
    prerender: {
      handleHttpError: 'warn'
    }
  }
};

export default config;