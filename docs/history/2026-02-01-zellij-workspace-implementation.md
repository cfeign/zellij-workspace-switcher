# Zellij 9-Tab Workspace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a Zellij workspace with 9 tabs - 1 grid view for monitoring 8 Claude Codes, and 8 project tabs each showing 1 Claude + 4 tools.

**Architecture:** Single layout file defines all 9 tabs. Tab 0 is the grid view (8 Claude panes in 2x4). Tabs 1-8 are project views (Claude 50% + 2x2 tool grid 50%). Keybindings use Alt+0-8 for direct tab navigation.

**Tech Stack:** Zellij terminal multiplexer, KDL configuration language

---

### Task 1: Create the Main Layout File

**Files:**
- Create: `~/.config/zellij/layouts/workspace.kdl`

**Step 1: Write the layout file**

Create the full 9-tab layout with all 40 panes:

```kdl
// Zellij 9-tab workspace: Grid view + 8 project views
// Alt+0 = grid (all 8 Claudes), Alt+1-8 = project tabs

layout {
    // Tab 0: Grid view - all 8 Claude Codes for monitoring
    tab name="grid" focus=true {
        pane split_direction="horizontal" {
            pane size="50%" split_direction="vertical" {
                pane name="claude-1" cwd="~/code/matrix-bot" command="~/.local/bin/claude"
                pane name="claude-2" cwd="~/code/playatime" command="~/.local/bin/claude"
                pane name="claude-3" cwd="~/code/treatmentplan" command="~/.local/bin/claude"
                pane name="claude-4" cwd="~/code/macbook-manager" command="~/.local/bin/claude"
            }
            pane size="50%" split_direction="vertical" {
                pane name="claude-5"
                pane name="claude-6"
                pane name="claude-7"
                pane name="claude-8"
            }
        }
    }

    // Tab 1: Project 1 - matrix-bot
    tab name="proj-1" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-1" cwd="~/code/matrix-bot" command="~/.local/bin/claude"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-1" cwd="~/code/matrix-bot" command="/opt/homebrew/bin/lazygit"
                    pane name="bv-1" cwd="~/code/matrix-bot" command="/opt/homebrew/bin/bv"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-1" cwd="~/code/matrix-bot"
                    pane name="adhoc-1" cwd="~/code/matrix-bot"
                }
            }
        }
    }

    // Tab 2: Project 2 - playatime
    tab name="proj-2" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-2" cwd="~/code/playatime" command="~/.local/bin/claude"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-2" cwd="~/code/playatime" command="/opt/homebrew/bin/lazygit"
                    pane name="bv-2" cwd="~/code/playatime" command="/opt/homebrew/bin/bv"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-2" cwd="~/code/playatime"
                    pane name="adhoc-2" cwd="~/code/playatime"
                }
            }
        }
    }

    // Tab 3: Project 3 - treatmentplan
    tab name="proj-3" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-3" cwd="~/code/treatmentplan" command="~/.local/bin/claude"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-3" cwd="~/code/treatmentplan" command="/opt/homebrew/bin/lazygit"
                    pane name="bv-3" cwd="~/code/treatmentplan" command="/opt/homebrew/bin/bv"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-3" cwd="~/code/treatmentplan"
                    pane name="adhoc-3" cwd="~/code/treatmentplan"
                }
            }
        }
    }

    // Tab 4: Project 4 - macbook-manager
    tab name="proj-4" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-4" cwd="~/code/macbook-manager" command="~/.local/bin/claude"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-4" cwd="~/code/macbook-manager" command="/opt/homebrew/bin/lazygit"
                    pane name="bv-4" cwd="~/code/macbook-manager" command="/opt/homebrew/bin/bv"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-4" cwd="~/code/macbook-manager"
                    pane name="adhoc-4" cwd="~/code/macbook-manager"
                }
            }
        }
    }

    // Tab 5: Project 5 - empty slot
    tab name="proj-5" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-5"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-5"
                    pane name="bv-5"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-5"
                    pane name="adhoc-5"
                }
            }
        }
    }

    // Tab 6: Project 6 - empty slot
    tab name="proj-6" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-6"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-6"
                    pane name="bv-6"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-6"
                    pane name="adhoc-6"
                }
            }
        }
    }

    // Tab 7: Project 7 - empty slot
    tab name="proj-7" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-7"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-7"
                    pane name="bv-7"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-7"
                    pane name="adhoc-7"
                }
            }
        }
    }

    // Tab 8: Project 8 - empty slot
    tab name="proj-8" {
        pane split_direction="horizontal" {
            pane size="50%" name="claude-8"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="git-8"
                    pane name="bv-8"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test-8"
                    pane name="adhoc-8"
                }
            }
        }
    }
}
```

**Step 2: Verify file was created**

Run: `ls -la ~/.config/zellij/layouts/workspace.kdl`
Expected: File exists with reasonable size (~4KB)

---

### Task 2: Update Zellij Config Keybindings

**Files:**
- Modify: `~/.config/zellij/config.kdl`

**Step 1: Remove swap layout keybindings and clean up tab navigation comments**

Find and remove these lines (around lines 34-36):
```kdl
// Swap layouts with Ctrl+Shift+p and Ctrl+Shift+n
bind "Ctrl Shift p" { PreviousSwapLayout; }
bind "Ctrl Shift n" { NextSwapLayout; }
```

Also remove the Alt+s swap popup binding (around lines 42-47):
```kdl
// Swap popup
bind "Alt s" {
    Run "zellij-swap" {
        floating true
        close_on_exit true
    };
}
```

**Step 2: Verify config is still valid**

Run: `~/.local/bin/zellij setup --check`
Expected: No errors (or just warnings about missing plugins)

---

### Task 3: Update zellij-dev Script

**Files:**
- Modify: `~/.local/bin/zellij-dev`

**Step 1: Change LAYOUT_FILE to point to new workspace.kdl**

Change line 6 from:
```bash
LAYOUT_FILE="$HOME/.config/zellij/layouts/workspace-simple.kdl"
```

To:
```bash
LAYOUT_FILE="$HOME/.config/zellij/layouts/workspace.kdl"
```

**Step 2: Verify script is executable and valid**

Run: `bash -n ~/.local/bin/zellij-dev && echo "Syntax OK"`
Expected: "Syntax OK"

---

### Task 4: Clean Up Old Layout Files

**Files:**
- Delete: `~/.config/zellij/layouts/workspace-simple.kdl`
- Delete: `~/.config/zellij/layouts/workspace-simple.swap.kdl`

**Step 1: Remove old files**

Run: `rm ~/.config/zellij/layouts/workspace-simple.kdl ~/.config/zellij/layouts/workspace-simple.swap.kdl`
Expected: No output (success)

**Step 2: Verify only workspace.kdl remains**

Run: `ls ~/.config/zellij/layouts/`
Expected: `workspace.kdl` (and possibly default.kdl if it exists)

---

### Task 5: Test the New Setup

**Step 1: Kill any existing Zellij session**

Run: `~/.local/bin/zellij delete-session workspace --force 2>/dev/null || true`
Expected: Session deleted or "session not found"

**Step 2: Launch new session**

Run: `~/.local/bin/zellij-dev`
Expected: Zellij launches with 9 tabs

**Step 3: Verify tab navigation**

Test these keybindings:
- `Alt+0` → Should show grid view (8 Claudes in 2x4)
- `Alt+1` → Should show proj-1 (Claude + 4 tools for matrix-bot)
- `Alt+2` → Should show proj-2 (Claude + 4 tools for playatime)
- `Alt+h/j/k/l` → Should navigate between panes within a tab

**Step 4: Verify session persistence**

1. Detach from session: `Ctrl+g` then close terminal
2. Reopen terminal and run: `~/.local/bin/zellij-dev`
3. Expected: Should reattach to existing session with all tabs intact

---

### Task 6: Commit Implementation

**Step 1: Stage the plan file**

Run: `cd ~/code/macbook-manager && git add docs/plans/2026-02-01-zellij-workspace-implementation.md`

**Step 2: Commit (skip bd hook if still broken)**

Run: `git commit --no-verify -m "Add Zellij 9-tab workspace implementation plan"`
Expected: Commit created

---

## Verification Checklist

- [ ] `~/.config/zellij/layouts/workspace.kdl` exists with 9 tabs
- [ ] `~/.config/zellij/config.kdl` has no swap layout bindings
- [ ] `~/.local/bin/zellij-dev` points to `workspace.kdl`
- [ ] Old `workspace-simple*.kdl` files are removed
- [ ] `Alt+0` shows grid view (8 Claudes)
- [ ] `Alt+1-8` each show Claude + 4 tools
- [ ] `Alt+h/j/k/l` navigates within tabs
- [ ] Session persists after terminal close/reopen
