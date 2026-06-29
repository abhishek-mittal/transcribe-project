# Handover — 2026-06-28 — claude-code

## Status
Done — tauri-mcp set up and verified for both Claude Code and GitHub Copilot.

## What changed this session
- Built and installed `dirvine/tauri-mcp` (https://mcpservers.org/servers/dirvine/tauri-mcp)
  via `cargo install --path .` from a shallow clone. Binary is now on PATH at
  `~/.cargo/bin/tauri-mcp` (v0.1.5). Not vendored into this repo — each contributor
  needs their own `cargo install tauri-mcp` (crates.io) or build from source.
- `.mcp.json` (new, repo root) — `mcpServers.tauri-mcp` → `tauri-mcp serve`. Read by
  Claude Code and the Copilot CLI (same `mcpServers` key convention).
- `.vscode/mcp.json` (new) — `servers.tauri-mcp`, `type: "stdio"` → `tauri-mcp serve`.
  Read by GitHub Copilot inside VS Code (different key name: `servers`, not
  `mcpServers`; VS Code ignores root `.mcp.json`).
- Both configs use `"command": "tauri-mcp"` (relies on PATH), not an absolute path —
  avoids hardcoding a machine-specific path into a committed file.

## Why tauri-mcp and what it gives agents
Exposes 12 MCP tools for testing/debugging this project's Tauri app directly: launch/stop
app, capture logs, screenshot the window, get window info, simulate keyboard/mouse input,
execute JS in the webview, list/call Tauri IPC commands, monitor CPU/memory. Useful for
agents verifying UI changes (e.g. the F14 picker layout carry-over noted in the prior
handover) without a human manually clicking through the app.

## Important implementation detail (don't re-investigate this)
The README make it look like `tauri-mcp serve --host --port` is an HTTP server, and warns
that Claude Desktop can't use the raw Rust binary (needs a Node.js DXT wrapper) due to a
known Claude-Desktop-specific disconnect bug. **Neither caveat applies here.** Read
`src/server.rs`: `serve()` always communicates over stdin/stdout JSON-RPC 2.0 regardless
of the host/port flags (they're vestigial). Claude Code and Copilot both speak stdio MCP
natively — no DXT, no Node wrapper needed. Verified with a live `initialize` handshake:
```
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}\n' | tauri-mcp serve
```
returned a correct MCP `initialize` response (capabilities, serverInfo, protocolVersion echo).

## Next action
Restart/reload Claude Code and VS Code (or run `Copilot: List MCP Servers` /
equivalent) in this project so each tool picks up the new MCP config, then try one
tauri-mcp tool (e.g. `take_screenshot` or `launch_app`) against the running Tauri app to
confirm the end-to-end agent → MCP server → Tauri app path works, not just the bare
`initialize` handshake.

## Do NOT
- Don't follow the README's Claude-Desktop DXT/Node-wrapper instructions for Claude Code
  or Copilot — that workaround is for Claude Desktop's extension system specifically and
  is unnecessary (and not even applicable) here.
- Don't hardcode an absolute `~/.cargo/bin/tauri-mcp` path into the committed configs —
  `tauri-mcp` on PATH is correct as long as cargo's bin dir is on PATH (it is, on this
  machine, and is the cargo-standard default).

## Open questions / blockers
- OpenCode's MCP config location/format wasn't set up — only Claude Code and Copilot
  were requested. If OpenCode parity is wanted later, check `.opencode/` config docs.
- (carried over) Smoke-test the F14 picker layout in the live UI — see archived handover
  `archive/2026-06-28-f14-ux-polish.md` for full detail; now possible to do via tauri-mcp
  instead of manual clicking, if useful.

## Related
- OpenSpec change: none
- __specs__ file: none
- _memory/rna-method/timeline.json updated: yes — new `recentDecisions[]` entry for
  the tauri-mcp setup
