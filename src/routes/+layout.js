// Disable SSR for the Tauri build. The frontend is a single-page app
// that lives entirely in the webview; we don't need server rendering.
//
// prerender = true + ssr = false ensures adapter-static emits a single
// index.html that the SvelteKit router hydrates client-side.

export const prerender = true;
export const ssr = false;
export const trailingSlash = 'always';