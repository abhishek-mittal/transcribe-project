# FIX-03 — Remove Native macOS Title Bar (Window Decorations)

## Priority
P2 — Cosmetic. Two title bars stack on top of each other.

---

## Current state
`tauri.conf.json` does not set `decorations` on the window. Tauri's default is `true` — the native macOS title bar (with traffic lights) is shown. The app's webview has its own custom topbar (`.desktop-topbar`, 52px, styled with drag region). The result is two title bars stacked: the native macOS strip at the very top, then the webview's topbar immediately below it. The app looks broken.

The webview topbar already handles dragging: `.desktop-topbar` has `-webkit-app-region: drag` and `.topbar-actions` has `-webkit-app-region: no-drag`. Tauri will automatically position the macOS traffic light buttons (close / minimise / fullscreen) in the top-left corner of the frameless window at the correct 8px inset.

---

## After state

| Location | Before | After |
|---|---|---|
| `tauri.conf.json` windows config | `decorations` key absent (defaults to true) | `"decorations": false` |
| App visual | Native title bar + webview topbar stacked | Single unified topbar, traffic lights in webview |

---

## Target file
`src-tauri/tauri.conf.json` — the `app.windows[0]` object.

---

## What to do
In `tauri.conf.json`, inside the `app.windows` array object, add one key:
```json
"decorations": false
```

The current windows object looks like:
```json
{
  "title": "Transcribe",
  "width": 1000,
  "height": 800,
  "minWidth": 800,
  "minHeight": 600,
  "resizable": true
}
```

After the change:
```json
{
  "title": "Transcribe",
  "width": 1000,
  "height": 800,
  "minWidth": 800,
  "minHeight": 600,
  "resizable": true,
  "decorations": false
}
```

No other files need to change. The webview's drag region and no-drag region are already correctly set.

---

## Acceptance criteria
1. Build and open `Transcribe.app`.
2. The native macOS title bar strip (the grey/white bar above the webview) is gone.
3. The red/yellow/green traffic light buttons appear in the top-left corner overlapping the webview's topbar at approximately 8px from the top-left.
4. Clicking and dragging the topbar area moves the window.
5. The theme toggle button (in `.topbar-actions`) remains clickable — the no-drag region is respected.
6. Window resize, minimise, fullscreen all work normally via the traffic lights.

---

## Note
This fix must be verified in the packaged `.app`, not in `npm run tauri:dev`, because dev mode sometimes handles decorations differently.
