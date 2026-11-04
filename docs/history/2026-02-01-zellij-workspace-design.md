# Zellij Multi-Project Workspace Design

**Date:** 2026-02-01
**Status:** Ready for implementation

## Overview

A Zellij workspace with 9 tabs for managing 8 parallel Claude Code projects. Tab 0 shows all 8 Claude Codes in a grid for monitoring. Tabs 1-8 each show one Claude Code with its companion tools (lazygit, bv, tests, adhoc).

## Architecture

### Tab Structure

| Tab | Name    | Contents                          | Keybinding |
|-----|---------|-----------------------------------|------------|
| 0   | grid    | 8 Claude Codes in 2x4 grid        | Alt+0      |
| 1   | proj-1  | Claude-1 + lazygit/bv/tests/adhoc | Alt+1      |
| 2   | proj-2  | Claude-2 + lazygit/bv/tests/adhoc | Alt+2      |
| 3   | proj-3  | Claude-3 + lazygit/bv/tests/adhoc | Alt+3      |
| 4   | proj-4  | Claude-4 + lazygit/bv/tests/adhoc | Alt+4      |
| 5   | proj-5  | Claude-5 + lazygit/bv/tests/adhoc | Alt+5      |
| 6   | proj-6  | Claude-6 + lazygit/bv/tests/adhoc | Alt+6      |
| 7   | proj-7  | Claude-7 + lazygit/bv/tests/adhoc | Alt+7      |
| 8   | proj-8  | Claude-8 + lazygit/bv/tests/adhoc | Alt+8      |

**Total panes:** 40 (8 Claude + 32 tools)

### Tab 0 - Grid View

```
┌─────────────┬─────────────┐
│  claude-1   │  claude-5   │
├─────────────┼─────────────┤
│  claude-2   │  claude-6   │
├─────────────┼─────────────┤
│  claude-3   │  claude-7   │
├─────────────┼─────────────┤
│  claude-4   │  claude-8   │
└─────────────┴─────────────┘
```

- Each pane: 50% width x 25% height
- Purpose: Quick scan of all Claude outputs

### Tabs 1-8 - Project View

```
┌──────────────────────┬────────────┬────────────┐
│                      │  lazygit   │    bv      │
│      claude-N        │   (25%)    │   (25%)    │
│       (50%)          ├────────────┼────────────┤
│                      │   tests    │   adhoc    │
│                      │   (25%)    │   (25%)    │
└──────────────────────┴────────────┴────────────┘
```

- Claude: 50% width, full height
- Tools: 2x2 grid in remaining 50%
- Top row: lazygit (git TUI), bv (file viewer) - auto-launch
- Bottom row: tests, adhoc - empty shells

### Project Directories

| Tab | Project Path                          |
|-----|---------------------------------------|
| 1   | ~/code/matrix-bot        |
| 2   | ~/code/playatime         |
| 3   | ~/code/treatmentplan     |
| 4   | ~/code/macbook-manager   |
| 5-8 | (empty shells, no cwd)                |

## Keybindings

### Tab Navigation
- `Alt+0` - Grid view (Tab 1)
- `Alt+1` through `Alt+8` - Project tabs (Tabs 2-9)

### Pane Navigation (within tab)
- `Alt+h/j/k/l` - Move focus left/down/up/right
- `Alt+f` - Toggle fullscreen for focused pane

### Other
- `Ctrl+g` - Lock/unlock mode
- `Ctrl+n` - New pane
- `Ctrl+x` - Close pane
- `Ctrl+p` - Resize mode
- `Ctrl+q` - Quit Zellij

## Files

### Create/Modify
- `~/.config/zellij/layouts/workspace.kdl` - Main layout with 9 tabs
- `~/.config/zellij/config.kdl` - Remove swap layout keybindings
- `~/.local/bin/zellij-dev` - Update LAYOUT_FILE path

### Remove
- `~/.config/zellij/layouts/workspace-simple.kdl`
- `~/.config/zellij/layouts/workspace-simple.swap.kdl`

## Session Persistence

Zellij's `session_serialization true` (already configured) saves:
- All 9 tabs and their pane arrangements
- Running commands in each pane
- Scroll history

Session survives terminal close and reopen via `zellij-dev` script.
