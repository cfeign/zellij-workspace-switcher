# Live Slot Swap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable swapping projects in/out of workspace slots without restarting Zellij — triggered by Alt+S keyboard shortcut opening an fzf picker.

**Architecture:** The existing `zellij-swap` fzf TUI selects a project+slot, then pipes a `swap-slot <n> <project-name> <project-path>` command to the WASM plugin via `zellij pipe`. The plugin closes the 5 old panes for that slot, opens 5 new floating panes (claude, lazygit, bv, test, adhoc) with the new project's cwd, renames them with the correct naming convention, and repositions them. The swap script also updates `projects.json` and regenerates the KDL layout so the next session restart matches.

**Tech Stack:** Rust (zellij-tile 0.43.1 WASM plugin), Bash (zellij-swap TUI), jq, fzf, Zellij KDL config

---

### Task 1: Add New Permissions to Plugin

**Files:**
- Modify: `<repo>/src/main.rs:59-63`

**Step 1: Write the permission request change**

In `main.rs`, update the `load()` function to request additional permissions needed for opening/closing panes:

```rust
fn load(&mut self, _configuration: BTreeMap<String, String>) {
    eprintln!("[ws] Plugin loading - v3 (live swap)");
    request_permission(&[
        PermissionType::ReadApplicationState,
        PermissionType::ChangeApplicationState,
        PermissionType::OpenTerminalsOrPlugins,
        PermissionType::RunCommands,
    ]);
    subscribe(&[EventType::PaneUpdate, EventType::PermissionRequestResult]);
    eprintln!("[ws] Plugin loaded");
}
```

**Step 2: Build to verify it compiles**

Run: `cd <repo> && cargo build --target wasm32-wasip1 --release 2>&1`
Expected: compiles successfully

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: request OpenTerminalsOrPlugins and RunCommands permissions for live swap"
```

---

### Task 2: Add `swap-slot` Command Handler to Plugin

**Files:**
- Modify: `<repo>/src/main.rs`

The plugin receives `swap-slot <slot> <project-name> <project-path>` via pipe. It must:
1. Close all 5 existing panes for that slot
2. Open 5 new floating panes with the new project's cwd and commands
3. The new panes get names matching the naming convention so `rebuild_slots()` picks them up on next PaneUpdate

**Step 1: Add the swap-slot command to `handle_command()`**

Add this match arm in `handle_command()` after the `["rescan"]` arm:

```rust
["swap-slot", slot_str, project_name, project_path] => {
    if let Ok(slot) = slot_str.parse::<u8>() {
        if (1..=8).contains(&slot) {
            eprintln!("[ws] Swapping slot {} -> {} ({})", slot, project_name, project_path);
            self.swap_slot(slot, project_name, project_path);
        } else {
            eprintln!("[ws] Invalid slot number: {}", slot);
        }
    }
}
```

Note: Since the project path may contain spaces, we need to handle that. Change the command parsing from `split_whitespace` to a custom split that respects the 4-part structure. Update `handle_command`:

```rust
fn handle_command(&mut self, command: &str) {
    let parts: Vec<&str> = command.trim().splitn(4, ' ').collect();
    match parts.as_slice() {
        ["grid"] | ["claude-only"] => {
            // ... existing code unchanged
        }
        ["focus", slot_str] => {
            // ... existing code unchanged
        }
        ["focus-pane", pane_type] => {
            // ... existing code unchanged
        }
        ["rescan"] => {
            // ... existing code unchanged
        }
        ["swap-slot", slot_str, rest] => {
            // rest = "project-name /path/to/project"
            // Split on last space? No — project names don't have spaces, paths might.
            // Format: "swap-slot <n> <name> <path>"
            // We used splitn(4, ' ') so rest contains everything after slot.
            // Need to split rest into name and path.
            if let Some(space_idx) = rest.find(' ') {
                let project_name = &rest[..space_idx];
                let project_path = &rest[space_idx + 1..];
                if let Ok(slot) = slot_str.parse::<u8>() {
                    if (1..=8).contains(&slot) {
                        eprintln!("[ws] Swapping slot {} -> {} ({})", slot, project_name, project_path);
                        self.swap_slot(slot, project_name, project_path);
                    }
                }
            }
        }
        _ => eprintln!("[ws] Unknown command: {:?}", parts),
    }
}
```

Wait — `splitn(4, ' ')` on `"swap-slot 6 test-watcher ~/code/test-watcher"` gives `["swap-slot", "6", "test-watcher", "~/code/test-watcher"]`. That's 4 parts. Let's use that directly:

```rust
fn handle_command(&mut self, command: &str) {
    // splitn(5, ' ') to handle "swap-slot <slot> <name> <path-with-possible-spaces>"
    let parts: Vec<&str> = command.trim().splitn(5, ' ').collect();
    match parts.as_slice() {
        ["grid"] | ["claude-only"] => {
            // ... existing
        }
        ["focus", slot_str] => {
            // ... existing
        }
        ["focus-pane", pane_type] => {
            // ... existing
        }
        ["rescan"] => {
            // ... existing
        }
        ["swap-slot", slot_str, project_name, project_path] => {
            if let Ok(slot) = slot_str.parse::<u8>() {
                if (1..=8).contains(&slot) {
                    eprintln!("[ws] Swapping slot {} -> {} ({})", slot, project_name, project_path);
                    self.swap_slot(slot, project_name, project_path);
                }
            }
        }
        _ => eprintln!("[ws] Unknown command: {:?}", parts),
    }
}
```

Hmm, but `splitn(5, ' ')` on `"focus 3"` still gives `["focus", "3"]` — fine. On `"swap-slot 6 test-watcher ~/my projects/foo"` it gives `["swap-slot", "6", "test-watcher", "~/my", "projects/foo"]` — that's 5 parts and won't match the 4-element pattern. Project paths with spaces are unlikely in this user's setup (all paths are `~/code/*`), but let's be safe. Use `splitn(4, ' ')` where the 4th element captures the entire remaining path:

```rust
// For swap-slot: "swap-slot 6 test-watcher /path/to/project"
// splitn(4, ' ') → ["swap-slot", "6", "test-watcher", "/path/to/project with spaces"]
```

This works. But then `["focus-pane", pane_type]` won't match from `splitn(4, ' ')` since "focus-pane claude" only has 2 parts — and `splitn` doesn't change that. OK, `splitn(4, ' ')` is fine for all existing commands too.

**Final implementation for handle_command:**

```rust
fn handle_command(&mut self, command: &str) {
    let parts: Vec<&str> = command.trim().splitn(4, ' ').collect();
    match parts.as_slice() {
        ["grid"] | ["claude-only"] => {
            if self.current_mode == Some(Mode::Grid) {
                eprintln!("[ws] Already in grid mode");
                return;
            }
            eprintln!("[ws] -> Grid mode");
            self.apply_grid_mode();
        }
        ["focus", slot_str] => {
            if let Ok(slot) = slot_str.parse::<u8>() {
                if (1..=8).contains(&slot) {
                    if self.current_mode == Some(Mode::Focus(slot)) {
                        eprintln!("[ws] Already focused on slot {}", slot);
                        return;
                    }
                    eprintln!("[ws] -> Focus slot {}", slot);
                    self.apply_focus_mode(slot);
                }
            }
        }
        ["focus-pane", pane_type] => {
            if let Some(Mode::Focus(slot)) = self.current_mode {
                if let Some(slot_panes) = self.slots.get(&slot) {
                    let pane_id = match *pane_type {
                        "claude" => slot_panes.claude,
                        "lazygit" => slot_panes.lazygit,
                        "bv" => slot_panes.bv,
                        "test" => slot_panes.test,
                        "adhoc" => slot_panes.adhoc,
                        _ => {
                            eprintln!("[ws] Unknown pane type: {}", pane_type);
                            return;
                        }
                    };
                    if let Some(id) = pane_id {
                        focus_pane_with_id(id, false);
                        eprintln!("[ws] Focused {} pane in slot {}", pane_type, slot);
                    } else {
                        eprintln!("[ws] Pane {} not found in slot {}", pane_type, slot);
                    }
                }
            } else {
                eprintln!("[ws] Not in focus mode - use Alt+1-8 first");
            }
        }
        ["rescan"] => {
            eprintln!("[ws] Rescan requested - resetting state");
            self.initialized = false;
            self.current_mode = None;
            self.apply_grid_mode();
            self.initialized = true;
        }
        ["swap-slot", slot_str, project_name, project_path] => {
            if let Ok(slot) = slot_str.parse::<u8>() {
                if (1..=8).contains(&slot) {
                    eprintln!("[ws] Swapping slot {} -> {} ({})", slot, project_name, project_path);
                    self.swap_slot(slot, project_name, project_path);
                }
            }
        }
        _ => eprintln!("[ws] Unknown command: {:?}", parts),
    }
}
```

**Step 2: Implement `swap_slot()` method**

Add to the `impl State` block:

```rust
use std::path::PathBuf;

/// Swap a slot to a new project: close old panes, open new ones
fn swap_slot(&mut self, slot: u8, project_name: &str, project_path: &str) {
    let path = PathBuf::from(project_path);

    // Step 1: Close existing panes for this slot
    if let Some(slot_panes) = self.slots.get(&slot) {
        let mut pane_ids = Vec::new();
        if let Some(id) = slot_panes.claude { pane_ids.push(id); }
        if let Some(id) = slot_panes.lazygit { pane_ids.push(id); }
        if let Some(id) = slot_panes.bv { pane_ids.push(id); }
        if let Some(id) = slot_panes.test { pane_ids.push(id); }
        if let Some(id) = slot_panes.adhoc { pane_ids.push(id); }

        if !pane_ids.is_empty() {
            eprintln!("[ws] Closing {} panes for slot {}", pane_ids.len(), slot);
            close_multiple_panes(pane_ids);
        }
    }

    // Step 2: Open new panes
    // Claude pane (command pane running claude-workspace)
    let claude_name = format!("claude-{}-{}", slot, project_name);
    let claude_cmd = CommandToRun {
        path: PathBuf::from("~/.local/bin/claude-workspace"),
        args: vec![],
        cwd: Some(path.clone()),
    };
    let grid_coords = self.build_grid_coordinates();
    let claude_coords = grid_coords.get(&slot).map(|c| self.coords_to_floating(c));

    let mut context = BTreeMap::new();
    context.insert("name".to_string(), claude_name);
    open_command_pane_floating(claude_cmd, claude_coords, context);

    // Lazygit pane
    let lazygit_cmd = CommandToRun {
        path: PathBuf::from("/opt/homebrew/bin/lazygit"),
        args: vec![],
        cwd: Some(path.clone()),
    };
    let mut ctx = BTreeMap::new();
    ctx.insert("name".to_string(), format!("lazygit-{}", slot));
    let tool_coords = Some(FloatingPaneCoordinates {
        x: Some(SplitSize::Percent(50)),
        y: Some(SplitSize::Percent(0)),
        width: Some(SplitSize::Percent(25)),
        height: Some(SplitSize::Percent(50)),
        pinned: None,
    });
    open_command_pane_floating(lazygit_cmd, tool_coords, ctx);

    // BV pane
    let bv_cmd = CommandToRun {
        path: PathBuf::from("/opt/homebrew/bin/bv"),
        args: vec![],
        cwd: Some(path.clone()),
    };
    let mut ctx = BTreeMap::new();
    ctx.insert("name".to_string(), format!("bv-{}", slot));
    let bv_coords = Some(FloatingPaneCoordinates {
        x: Some(SplitSize::Percent(75)),
        y: Some(SplitSize::Percent(0)),
        width: Some(SplitSize::Percent(25)),
        height: Some(SplitSize::Percent(50)),
        pinned: None,
    });
    open_command_pane_floating(bv_cmd, bv_coords, ctx);

    // Test pane (fish shell)
    let test_cmd = CommandToRun {
        path: PathBuf::from("/opt/homebrew/bin/fish"),
        args: vec![],
        cwd: Some(path.clone()),
    };
    let mut ctx = BTreeMap::new();
    ctx.insert("name".to_string(), format!("test-{}", slot));
    let test_coords = Some(FloatingPaneCoordinates {
        x: Some(SplitSize::Percent(50)),
        y: Some(SplitSize::Percent(50)),
        width: Some(SplitSize::Percent(25)),
        height: Some(SplitSize::Percent(50)),
        pinned: None,
    });
    open_command_pane_floating(test_cmd, test_coords, ctx);

    // Adhoc pane (fish shell)
    let adhoc_cmd = CommandToRun {
        path: PathBuf::from("/opt/homebrew/bin/fish"),
        args: vec![],
        cwd: Some(path.clone()),
    };
    let mut ctx = BTreeMap::new();
    ctx.insert("name".to_string(), format!("adhoc-{}", slot));
    let adhoc_coords = Some(FloatingPaneCoordinates {
        x: Some(SplitSize::Percent(75)),
        y: Some(SplitSize::Percent(50)),
        width: Some(SplitSize::Percent(25)),
        height: Some(SplitSize::Percent(50)),
        pinned: None,
    });
    open_command_pane_floating(adhoc_cmd, adhoc_coords, ctx);

    // Remove old slot data — will be rebuilt on next PaneUpdate
    self.slots.remove(&slot);

    eprintln!("[ws] Slot {} swap initiated — new panes spawning", slot);
}
```

**Important caveat:** The `context` BTreeMap with `"name"` key is how `open_command_pane_floating` sets the pane title. If this doesn't work (the docs aren't 100% clear on this), we'll need to use `rename_terminal_pane` after the pane appears in the next PaneUpdate. We'll verify in Task 3.

**Step 3: Add necessary use imports at the top of the file**

The file already imports `zellij_tile::prelude::*` which should include `CommandToRun`, `close_multiple_panes`, `open_command_pane_floating`. Also add `use std::path::PathBuf;` at the top.

**Step 4: Build to verify it compiles**

Run: `cd <repo> && cargo build --target wasm32-wasip1 --release 2>&1`
Expected: compiles successfully

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: add swap-slot command for live project swapping"
```

---

### Task 3: Verify Pane Naming Works (or add rename fallback)

**Files:**
- Possibly modify: `<repo>/src/main.rs`

The `open_command_pane_floating` context map may or may not set the pane title via the `"name"` key. We need to test this manually.

**Step 1: Deploy the plugin and test manually**

```bash
cp <repo>/target/wasm32-wasip1/release/workspace_switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
```

Then from a terminal inside the Zellij session:
```bash
zellij pipe --plugin "file:$HOME/.config/zellij/plugins/workspace_switcher.wasm" -- "swap-slot 8 test-project ~/code/macbook-manager"
```

Check if the new panes appear with correct titles (e.g., `claude-8-test-project`).

**Step 2: If names don't work, add rename fallback**

If panes spawn with generic names, we need to track "pending renames" and apply them when new unknown panes appear in the next `PaneUpdate`. Add to State:

```rust
/// Panes waiting to be renamed (mapped by expected cwd or spawn order)
pending_renames: Vec<(String, String)>,  // (expected_partial_title, desired_name)
```

And in `update_pane_map`, after rebuilding slots, check for panes that match pending renames and call `rename_terminal_pane(id, name)`.

This is conditional on Step 1 results — skip if naming works via context.

**Step 3: Commit if changes were needed**

```bash
git add src/main.rs
git commit -m "fix: add rename fallback for spawned panes"
```

---

### Task 4: Update `zellij-swap` to Pipe to Plugin Instead of Just Updating JSON

**Files:**
- Modify: `~/.local/bin/zellij-swap`

Currently the script updates `projects.json` and tells the user to restart. We need it to also:
1. Pipe the `swap-slot` command to the plugin for live swap
2. Regenerate the KDL layout for next restart consistency

**Step 1: Update the swap script**

Replace the end of `~/.local/bin/zellij-swap` (after `jq` updates `projects.json`):

```bash
# Update slot in projects.json
jq ".slots[\"$key\"] = \"$proj_name\"" "$CONFIG_FILE" > "$CONFIG_FILE.tmp" && mv "$CONFIG_FILE.tmp" "$CONFIG_FILE"

# Get project path
proj_path=$(jq -r --arg name "$proj_name" '.projects[] | select(.name == $name) | .path' "$CONFIG_FILE")

if [[ -z "$proj_path" || "$proj_path" == "null" ]]; then
    echo "Error: Could not find path for project $proj_name"
    sleep 2
    exit 1
fi

echo "Assigned $proj_name to slot $key"

# Live swap: pipe command to plugin
echo "Swapping live..."
zellij pipe --plugin "file:$HOME/.config/zellij/plugins/workspace_switcher.wasm" -- "swap-slot $key $proj_name $proj_path"

# Regenerate layout for next restart consistency
if [[ -x "$HOME/scripts/zellij-workspace-switcher/scripts/generate-layout.sh" ]]; then
    "$HOME/scripts/zellij-workspace-switcher/scripts/generate-layout.sh" > /dev/null 2>&1
fi

echo "Done!"
sleep 1
```

**Step 2: Test the full flow manually**

1. Press Alt+S (once wired) or run `zellij-swap` from a terminal
2. Select a project from the fzf list
3. Press a slot number (e.g., 8)
4. Verify the old panes close and new ones appear
5. Verify `projects.json` was updated
6. Verify the layout KDL was regenerated

**Step 3: Commit**

```bash
cd ~/.local/bin && git add zellij-swap  # or wherever this is tracked
```

Actually `zellij-swap` may not be in a git repo. Just save it.

---

### Task 5: Update `generate-layout.sh` to Generate Floating Pane Layout

**Files:**
- Modify: `<repo>/scripts/generate-layout.sh`

The current `generate-layout.sh` generates a **tiled** layout (split_direction=horizontal), but `workspace-minimal.kdl` uses **floating panes**. The generator needs to match the actual layout format.

**Step 1: Rewrite generate-layout.sh to output floating pane layout**

```bash
#!/bin/bash
# Generates workspace.kdl from projects.json (floating pane layout)
set -e

PROJECTS_JSON="$HOME/.config/zellij/projects.json"
OUTPUT_FILE="$HOME/.config/zellij/layouts/workspace-minimal.kdl"

if [ ! -f "$PROJECTS_JSON" ]; then
    echo "Error: $PROJECTS_JSON not found"
    exit 1
fi

cat > "$OUTPUT_FILE" << 'HEADER'
// Generated by workspace-switcher/scripts/generate-layout.sh
// Regenerate with: ./scripts/generate-layout.sh

layout {
    tab name="workspace" focus=true hide_floating_panes=true {
        // Plugin pane (minimal size) - stays in fixed position
        pane size=1 borderless=true {
            plugin location="file:~/.config/zellij/plugins/workspace_switcher.wasm"
        }

        // Empty pane (required for tab to function properly)
        pane

        floating_panes {
HEADER

# Grid coordinates for claude panes (2x4 grid)
declare -a X_PCT=("0" "25" "50" "75" "0" "25" "50" "75")
declare -a Y_PCT=("0" "0" "0" "0" "50" "50" "50" "50")

for slot in {1..8}; do
    idx=$((slot - 1))
    x="${X_PCT[$idx]}"
    y="${Y_PCT[$idx]}"

    project_name=$(jq -r ".slots[\"$slot\"] // \"empty\"" "$PROJECTS_JSON")

    if [ "$project_name" != "null" ] && [ "$project_name" != "empty" ]; then
        project_path=$(jq -r --arg name "$project_name" '.projects[] | select(.name == $name) | .path' "$PROJECTS_JSON")
    else
        project_name="empty"
        project_path=""
    fi

    # Claude pane
    echo "            // Slot $slot: $project_name" >> "$OUTPUT_FILE"
    if [ -n "$project_path" ] && [ "$project_path" != "null" ]; then
        cat >> "$OUTPUT_FILE" << PANE
            pane name="claude-${slot}-${project_name}" cwd="${project_path}" command="~/.local/bin/claude-workspace" {
                x "${x}%"; y "${y}%"; width "25%"; height "50%"
            }
PANE
    else
        cat >> "$OUTPUT_FILE" << PANE
            pane name="claude-${slot}-${project_name}" {
                x "${x}%"; y "${y}%"; width "25%"; height "50%"
            }
PANE
    fi

    # Tool panes (lazygit, bv, test, adhoc)
    if [ -n "$project_path" ] && [ "$project_path" != "null" ]; then
        cat >> "$OUTPUT_FILE" << PANE
            pane name="lazygit-${slot}" cwd="${project_path}" command="/opt/homebrew/bin/lazygit" {
                x "50%"; y "0%"; width "25%"; height "50%"
            }
            pane name="bv-${slot}" cwd="${project_path}" command="/opt/homebrew/bin/bv" {
                x "75%"; y "0%"; width "25%"; height "50%"
            }
            pane name="test-${slot}" cwd="${project_path}" command="/opt/homebrew/bin/fish" {
                x "50%"; y "50%"; width "25%"; height "50%"
            }
            pane name="adhoc-${slot}" cwd="${project_path}" command="/opt/homebrew/bin/fish" {
                x "75%"; y "50%"; width "25%"; height "50%"
            }
PANE
    else
        cat >> "$OUTPUT_FILE" << PANE
            pane name="lazygit-${slot}" {
                x "50%"; y "0%"; width "25%"; height "50%"
            }
            pane name="bv-${slot}" {
                x "75%"; y "0%"; width "25%"; height "50%"
            }
            pane name="test-${slot}" command="/opt/homebrew/bin/fish" {
                x "50%"; y "50%"; width "25%"; height "50%"
            }
            pane name="adhoc-${slot}" command="/opt/homebrew/bin/fish" {
                x "75%"; y "50%"; width "25%"; height "50%"
            }
PANE
    fi

    echo "" >> "$OUTPUT_FILE"
done

cat >> "$OUTPUT_FILE" << 'FOOTER'
        }
    }
}
FOOTER

echo "Generated: $OUTPUT_FILE"
```

**Step 2: Test it generates valid KDL**

Run: `cd <repo> && ./scripts/generate-layout.sh`
Then: `diff ~/.config/zellij/layouts/workspace-minimal.kdl ~/.config/zellij/layouts/workspace-minimal.kdl.bak`
Verify the structure matches the current hand-maintained layout.

**Step 3: Commit**

```bash
git add scripts/generate-layout.sh
git commit -m "feat: rewrite layout generator for floating pane format"
```

---

### Task 6: Wire Alt+S Keybinding to Launch zellij-swap

**Files:**
- Modify: `~/.config/zellij/config.kdl`

**Step 1: Add the keybinding**

In `config.kdl`, inside the `shared_except "locked"` keybinds block, add:

```kdl
bind "Alt s" {
    Run "bash" "-c" "$HOME/.local/bin/zellij-swap" {
        in_place true
    }
}
```

Wait — `Run` with `in_place` opens a command pane replacing the focused pane, which would disrupt the layout. Better to use a floating pane:

```kdl
bind "Alt s" {
    Run "bash" "-c" "$HOME/.local/bin/zellij-swap" {
        floating true
    }
}
```

Actually, in Zellij KDL keybinding config, the action is:

```kdl
bind "Alt s" { LaunchOrFocusPlugin "..." }
```

No — `zellij-swap` is a bash script, not a plugin. The correct action is:

```kdl
bind "Alt s" {
    NewFloatingPane {
        command "bash"
        args "-c" "$HOME/.local/bin/zellij-swap"
    }
}
```

Hmm, the KDL keybinding syntax for running commands is:

```kdl
bind "Alt s" {
    Run "bash" "-c" "~/.local/bin/zellij-swap" {
        direction "Floating"
    }
}
```

Actually checking the Zellij docs — the correct keybinding action syntax is one of the "Possible Actions". The relevant one is `NewFloatingPane`. Let's verify and use:

```kdl
bind "Alt s" {
    NewFloatingPane {
    }
}
```

No, that just opens a blank pane. We need `Run` which is a CLI action, not a keybinding action. The keybinding-compatible approach is to use `MessagePlugin` to pipe a command, but `zellij-swap` is interactive (fzf), so it needs a real terminal pane.

The simplest approach: use `zellij action new-floating-pane` with a command. But that's a CLI action, not a keybinding.

**Alternative:** Use a `LaunchPlugin` with a helper plugin that runs the script. Too complex.

**Simplest working approach:** Use the `Run` action which IS available in keybindings:

```kdl
bind "Alt s" {
    Run "bash" "-c" "~/.local/bin/zellij-swap" {
        floating true
        close_on_exit true
    }
}
```

If that syntax doesn't work, the fallback is:

```kdl
bind "Alt s" {
    NewFloatingPane command="bash" {
        args "-c" "~/.local/bin/zellij-swap"
        close_on_exit true
    }
}
```

We'll need to test which syntax Zellij actually accepts. Check the existing keybindings in config.kdl for pattern guidance, then test.

**Step 2: Reload Zellij config**

Zellij hot-reloads config on save, so just saving the file should apply the keybinding.

**Step 3: Test pressing Alt+S**

Press Alt+S in any pane. An fzf picker should appear in a floating pane showing all slots and available projects.

**Step 4: Commit**

```bash
git add ~/.config/zellij/config.kdl
```

---

### Task 7: Sync projects.json with Actual Layout

**Files:**
- Modify: `~/.config/zellij/projects.json`

The current `projects.json` only has 4 projects (slots 1-4), but `workspace-minimal.kdl` has 7 projects hardcoded (slots 1-7). We need to register the missing projects and sync slots.

**Step 1: Add missing projects to projects.json**

```bash
# Add missing projects
jq '.projects += [
  {"name": "coryfeign", "path": "~/code/coryfeign.com"},
  {"name": "sox-rec", "path": "~/code/SoX-rec"},
  {"name": "waitlist-tracker", "path": "~/code/waitlist-tracker"}
] | .slots["5"] = "coryfeign" | .slots["6"] = "sox-rec" | .slots["7"] = "waitlist-tracker"' \
  ~/.config/zellij/projects.json > /tmp/zp.json && mv /tmp/zp.json ~/.config/zellij/projects.json
```

**Step 2: Verify**

Run: `/zellij-project list` or `jq . ~/.config/zellij/projects.json`

---

### Task 8: End-to-End Test

**No files to modify — this is a manual verification task.**

**Step 1: Deploy updated plugin**

```bash
cp <repo>/target/wasm32-wasip1/release/workspace_switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
```

**Step 2: Restart Zellij session**

```bash
zellij-workspace nuke
# Then relaunch via Ghostty Zellij 2.app
```

**Step 3: Test swap flow**

1. Press Alt+S → fzf picker appears
2. Select an available project (or one from another slot)
3. Press a slot number
4. Verify: old panes close, new panes appear with correct names
5. Press Alt+0 → grid mode shows updated slot
6. Press Alt+<slot> → focus mode shows new project's tools
7. Verify `projects.json` reflects the change
8. Verify `workspace-minimal.kdl` was regenerated

**Step 4: Test edge cases**

- Swap while focused on the slot being swapped (should return to grid, then swap)
- Swap an empty slot (slot 8)
- Swap a slot to a project already in another slot (should that be allowed? Currently yes — produces duplicates)

---

## Risk Notes

1. **Pane naming via context:** The `open_command_pane_floating` context `"name"` key may not set the pane title. If not, Task 3's rename fallback is needed. This is the highest-risk item.

2. **fzf in floating pane:** The fzf picker running inside a Zellij floating pane should work, but if the pane is too small or fzf doesn't render properly, we may need to adjust the floating pane size.

3. **Permission prompt:** Adding `OpenTerminalsOrPlugins` and `RunCommands` permissions will trigger a Zellij permission prompt on first plugin load. User must approve.

4. **Timing:** After `close_multiple_panes`, there may be a brief delay before new panes can be opened. If panes fail to spawn, we may need to add a small delay or handle it reactively in `PaneUpdate`.
