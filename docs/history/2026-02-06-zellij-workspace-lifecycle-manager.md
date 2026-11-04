# Zellij Workspace Lifecycle Manager

**Date:** 2026-02-06
**Status:** Approved

## Problem

Closing Ghostty during a bad Zellij state (command pane in EXIT CODE state, rendering stall) can leave dozens of orphaned processes (lazygit, bv, claude, fish) with PPID=1. Recovery requires manually hunting and killing PIDs. There's no cleanup-on-launch, no emergency reset, and no visibility into process health.

## Design

Single script `~/.local/bin/zellij-workspace` with three subcommands:

### `zellij-workspace start`

Called by Ghostty Zellij 2.app launcher. Does four things in order:

1. **Reap orphans** — Find processes where PPID=1 and comm matches lazygit, bv, claude, or fish. SIGTERM, wait 2s, SIGKILL survivors. Print summary if any killed. Silent if none.
2. **Clean dead sessions** — Delete any EXITED zellij sessions.
3. **Create or attach** — If `workspace2` is active, attach. Otherwise create from layout.
4. **HUP trap** — `trap '' HUP` so server survives Ghostty closing.

### `zellij-workspace nuke`

Manual emergency reset. Runnable from any terminal.

1. `zellij kill-all-sessions --yes` — graceful Zellij shutdown
2. Wait 3s for child process cleanup
3. Force-kill remaining workspace processes (lazygit, bv, claude, fish with PPID=1)
4. Force-kill remaining zellij-server processes
5. Print summary

### `zellij-workspace status`

Health check showing: session state, process counts by type, orphan count, suggestion if unhealthy.

### Config change

Add `on_force_close "detach"` to `~/.config/zellij/config.kdl` — makes Zellij detach cleanly on unexpected terminal close instead of trying to tear down 40 processes simultaneously.

## Implementation Tasks

### Task 1: Create `zellij-workspace` script

Create `~/.local/bin/zellij-workspace` with start/nuke/status subcommands.

**Orphan detection logic:**
```
ps -eo pid,ppid,comm | awk where PPID=1 and comm in (lazygit, bv, claude, fish)
```

Note on fish: only kill fish processes whose PPID=1 (orphaned from dead zellij). Active user fish shells will have a real parent.

**start subcommand:**
- Reap orphans (SIGTERM, sleep 2, SIGKILL survivors)
- Delete EXITED sessions
- `trap '' HUP`
- Create or attach workspace2

**nuke subcommand:**
- `zellij kill-all-sessions --yes`
- sleep 3
- Kill remaining workspace processes
- Kill remaining zellij-server processes
- Summary output

**status subcommand:**
- Parse `zellij list-sessions` for session state
- Count processes by type (active vs orphaned)
- Print formatted status
- Suggest nuke if orphans detected

### Task 2: Update Ghostty Zellij 2.app launcher

Change `/Applications/Ghostty Zellij 2.app/Contents/MacOS/Ghostty Zellij 2` from:
```bash
exec /Applications/Ghostty.app/Contents/MacOS/ghostty -e ~/.local/bin/zellij-dev2
```
To:
```bash
exec /Applications/Ghostty.app/Contents/MacOS/ghostty -e ~/.local/bin/zellij-workspace start
```

### Task 3: Add `on_force_close "detach"` to config.kdl

Add the line to `~/.config/zellij/config.kdl` after the `session_serialization false` line.

### Task 4: Deprecate zellij-dev2

Add a comment header to `~/.local/bin/zellij-dev2`:
```bash
# DEPRECATED: Use zellij-workspace instead. Kept as backup.
```

## Files

| File | Action |
|---|---|
| `~/.local/bin/zellij-workspace` | Create |
| `/Applications/Ghostty Zellij 2.app/Contents/MacOS/Ghostty Zellij 2` | Edit |
| `~/.config/zellij/config.kdl` | Edit (add on_force_close) |
| `~/.local/bin/zellij-dev2` | Edit (deprecation comment) |
