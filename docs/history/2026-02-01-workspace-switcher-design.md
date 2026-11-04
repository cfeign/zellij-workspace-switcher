# Zellij Workspace Switcher Plugin Design

**Date:** 2026-02-01
**Status:** Ready for implementation

## Overview

A Zellij plugin that manages workspace visibility within a single tab. Allows instant switching between a grid view (all 8 Claude panes visible) and focus view (one Claude + 4 tool panes visible) using `hide_pane_with_id()` / `show_pane_with_id()` for true invisibility.

## Architecture

```
┌─────────────────┐     generates     ┌──────────────────┐
│ projects.json   │ ───────────────►  │ workspace.kdl    │
│ (registry)      │   (script)        │ (40 panes)       │
└─────────────────┘                   └────────┬─────────┘
                                               │ launches
                                               ▼
                                      ┌──────────────────┐
                                      │ Zellij session   │
                                      │ + plugin loaded  │
                                      └────────┬─────────┘
                                               │
                            ┌──────────────────┼──────────────────┐
                            ▼                  ▼                  ▼
                       Alt+0              Alt+1-8            PaneUpdate
                       (grid)             (focus)            (builds map)
```

### Components

| Component | Location | Responsibility |
|-----------|----------|----------------|
| **projects.json** | `~/.config/zellij/projects.json` | Source of truth for slot→project mapping |
| **Layout generator** | `zellij-workspace-switcher/scripts/` | Reads registry, generates `workspace.kdl` |
| **workspace.kdl** | `~/.config/zellij/layouts/workspace.kdl` | Defines all 40 panes with names and cwds |
| **workspace-switcher plugin** | `~/.config/zellij/plugins/workspace-switcher.wasm` | Shows/hides panes based on mode |
| **Keybind config** | `~/.config/zellij/config.kdl` | Maps Alt+0-8 to MessagePlugin commands |

## Pane Structure

### Naming Convention

| Pane Type | Display Name | Identification |
|-----------|--------------|----------------|
| Claude | `claude-{slot}-{project}` | Parse slot from name |
| Git | `lazygit` | Sibling of Claude in layout tree |
| File viewer | `bv` | Sibling of Claude in layout tree |
| Tests | `test` | Sibling of Claude in layout tree |
| Ad hoc | `adhoc` | Sibling of Claude in layout tree |

**Total: 40 panes** (8 slots × 5 panes each)

### Layout Structure

Each slot's panes are grouped in a container in the KDL layout:

```kdl
layout {
    tab name="workspace" {
        // Slot 1
        pane name="claude-1-matrix-bot" cwd="~/code/matrix-bot"
        pane name="lazygit" cwd="~/code/matrix-bot" command="lazygit"
        pane name="bv" cwd="~/code/matrix-bot" command="bv"
        pane name="test" cwd="~/code/matrix-bot"
        pane name="adhoc" cwd="~/code/matrix-bot"

        // Slot 2
        pane name="claude-2-treatmentplan" cwd="~/code/treatmentplan"
        // ... etc
    }
}
```

Plugin identifies tool panes by their position relative to Claude panes in the layout tree.

## Modes

### Grid Mode (Alt+0)

```
┌──────────────────────┬──────────────────────┬──────────────────────┬──────────────────────┐
│claude-1-matrix-bot   │claude-2-playatime    │claude-3-treatmentplan│claude-4-macbook-mgr  │
├──────────────────────┼──────────────────────┼──────────────────────┼──────────────────────┤
│claude-5-empty        │claude-6-empty        │claude-7-empty        │claude-8-empty        │
└──────────────────────┴──────────────────────┴──────────────────────┴──────────────────────┘
```

- **Visible:** 8 Claude panes
- **Hidden:** 32 tool panes

### Focus Mode (Alt+N)

```
┌───────────────────┬─────────┬─────────┐
│                   │ lazygit │   bv    │
│  claude-N-project ├─────────┼─────────┤
│                   │  test   │  adhoc  │
└───────────────────┴─────────┴─────────┘
```

- **Visible:** 1 Claude pane + 4 tool panes (for slot N)
- **Hidden:** 7 Claude panes + 28 tool panes

## Plugin Implementation

### Language & Location

- **Language:** Rust (using `zellij-tile` crate)
- **Repository:** `<repo>/`

### Project Structure

```
zellij-workspace-switcher/
├── Cargo.toml
├── src/
│   └── main.rs
├── scripts/
│   └── generate-layout.sh
└── README.md
```

### Core Data Structures

```rust
struct State {
    pane_map: HashMap<PaneId, PaneInfo>,  // All tracked panes
    slots: HashMap<u8, SlotPanes>,         // Slot # → its panes
    current_mode: Mode,
}

struct SlotPanes {
    claude: PaneId,
    tools: Vec<PaneId>,  // lazygit, bv, test, adhoc
}

enum Mode {
    Grid,
    Focus(u8),  // slot number 1-8
}
```

### Lifecycle

1. **`load()`** - Subscribe to `PaneUpdate`, request permissions
2. **`update(PaneUpdate)`** - Build/update `pane_map` and `slots` from pane info
3. **`update(CustomMessage)`** - Handle mode switch commands

### Key API Calls

| Action | Zellij API |
|--------|------------|
| Hide pane | `hide_pane_with_id(pane_id)` |
| Show pane | `show_pane_with_id(pane_id, should_float, should_focus)` |
| Get pane info | Subscribe to `PaneUpdate` events |

### Permissions

- `ChangeApplicationState` - to show/hide panes
- `ReadApplicationState` - to get pane info

## Keybindings

Add to `~/.config/zellij/config.kdl`:

```kdl
keybinds {
    shared {
        bind "Alt 0" { MessagePlugin "workspace-switcher" "grid"; }
        bind "Alt 1" { MessagePlugin "workspace-switcher" "focus 1"; }
        bind "Alt 2" { MessagePlugin "workspace-switcher" "focus 2"; }
        bind "Alt 3" { MessagePlugin "workspace-switcher" "focus 3"; }
        bind "Alt 4" { MessagePlugin "workspace-switcher" "focus 4"; }
        bind "Alt 5" { MessagePlugin "workspace-switcher" "focus 5"; }
        bind "Alt 6" { MessagePlugin "workspace-switcher" "focus 6"; }
        bind "Alt 7" { MessagePlugin "workspace-switcher" "focus 7"; }
        bind "Alt 8" { MessagePlugin "workspace-switcher" "focus 8"; }
    }
}
```

## Layout Generator

Script reads `projects.json` and generates `workspace.kdl`.

**Input:** `~/.config/zellij/projects.json`
```json
{
  "projects": [
    {"name": "matrix-bot", "path": "~/code/matrix-bot"},
    {"name": "treatmentplan", "path": "~/code/treatmentplan"}
  ],
  "slots": {
    "1": "matrix-bot",
    "2": "treatmentplan",
    "3": null,
    ...
  }
}
```

**Output:** `~/.config/zellij/layouts/workspace.kdl` with all 40 panes.

Run manually when slot assignments change.

## MVP Scope

**In scope:**
- [x] Grid mode (Alt+0) - 8 Claude panes visible
- [x] Focus mode (Alt+1-8) - 1 Claude + 4 tools visible
- [x] True invisibility via `hide_pane_with_id()` / `show_pane_with_id()`
- [x] Layout generator from registry
- [x] Keybind configuration

**Out of scope (future):**
- [ ] Swap popup (Alt+s) for reassigning slots interactively
- [ ] On-demand pane creation (panes that don't exist yet)
- [ ] Status bar indicator showing current mode/workspace
- [ ] Fuzzy finder for workspace selection

## Files to Create/Modify

### Create

| File | Purpose |
|------|---------|
| `<repo>/Cargo.toml` | Rust project manifest |
| `<repo>/src/main.rs` | Plugin implementation |
| `<repo>/scripts/generate-layout.sh` | Layout generator |
| `~/.config/zellij/layouts/workspace.kdl` | Generated layout (40 panes) |

### Modify

| File | Change |
|------|--------|
| `~/.config/zellij/config.kdl` | Add Alt+0-8 keybindings |

## Testing Plan

1. Build plugin: `cargo build --release`
2. Copy WASM to plugins dir
3. Generate layout from current `projects.json`
4. Launch Zellij with new layout
5. Verify Alt+0 shows grid (8 Claudes)
6. Verify Alt+1 shows focus (Claude-1 + 4 tools)
7. Verify other panes are truly hidden (not visible anywhere)
8. Verify switching back and forth preserves pane state
