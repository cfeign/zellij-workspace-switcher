# Workspace Switcher Plugin Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Zellij plugin that switches between grid view (8 Claude panes) and focus view (1 Claude + 4 tools) using hide/show pane commands.

**Architecture:** Rust plugin using `zellij-tile` crate. Subscribes to `PaneUpdate` events to track pane IDs by title. Handles `MessagePlugin` keybind commands to switch modes. Uses `hide_pane_with_id()` / `show_pane_with_id()` for visibility.

**Tech Stack:** Rust, zellij-tile 0.41.1, WASM target

---

### Task 1: Create Project Structure

**Files:**
- Create: `<repo>/Cargo.toml`
- Create: `<repo>/src/main.rs`
- Create: `<repo>/.gitignore`

**Step 1: Create the directory**

```bash
mkdir -p <repo>/src
```

**Step 2: Create Cargo.toml**

```toml
[package]
name = "workspace-switcher"
version = "0.1.0"
edition = "2021"

[dependencies]
zellij-tile = "0.41.1"

[lib]
crate-type = ["cdylib"]
path = "src/main.rs"
```

**Step 3: Create .gitignore**

```
/target
Cargo.lock
*.wasm
```

**Step 4: Create minimal main.rs skeleton**

```rust
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {}
    fn update(&mut self, _event: Event) -> bool { false }
    fn render(&mut self, _rows: usize, _cols: usize) {}
}
```

**Step 5: Initialize git repo**

```bash
cd <repo>
git init
git add .
git commit -m "Initial project structure"
```

---

### Task 2: Add Pane Tracking State

**Files:**
- Modify: `<repo>/src/main.rs`

**Step 1: Add required imports and state structs**

Replace entire `main.rs` with:

```rust
use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

/// Panes belonging to a single workspace slot
#[derive(Debug, Default, Clone)]
struct SlotPanes {
    claude: Option<PaneId>,
    tools: Vec<PaneId>,  // lazygit, bv, test, adhoc
}

/// Current display mode
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Grid,
    Focus(u8),  // slot number 1-8
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Grid
    }
}

#[derive(Default)]
struct State {
    /// Map from pane title to pane ID
    pane_ids: HashMap<String, PaneId>,
    /// Map from slot number (1-8) to its panes
    slots: HashMap<u8, SlotPanes>,
    /// Current display mode
    mode: Mode,
    /// Whether we've done initial setup
    initialized: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[EventType::PaneUpdate, EventType::PermissionRequestResult]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(pane_manifest) => {
                self.update_pane_map(pane_manifest);
                if !self.initialized {
                    self.initialized = true;
                    self.apply_mode();
                }
                true
            }
            Event::PermissionRequestResult(_) => {
                // Permissions granted, we'll get PaneUpdate next
                false
            }
            _ => false,
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if let Some(payload) = pipe_message.payload {
            self.handle_command(&payload);
            true
        } else {
            false
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        // Plugin is invisible - no UI needed
    }
}

impl State {
    fn update_pane_map(&mut self, manifest: PaneManifest) {
        self.pane_ids.clear();
        self.slots.clear();

        for (_tab_index, panes) in manifest.panes {
            for pane in panes {
                let pane_id = if pane.is_plugin {
                    PaneId::Plugin(pane.id)
                } else {
                    PaneId::Terminal(pane.id)
                };

                // Store by title
                self.pane_ids.insert(pane.title.clone(), pane_id);

                // Parse claude panes to determine slot membership
                if pane.title.starts_with("claude-") {
                    if let Some(slot) = self.parse_slot_from_claude_title(&pane.title) {
                        let slot_panes = self.slots.entry(slot).or_default();
                        slot_panes.claude = Some(pane_id);
                    }
                }
            }
        }

        // Assign tool panes to slots based on their position relative to claude panes
        // For now, we'll use a simpler approach: match by cwd or just track all tools
        self.assign_tool_panes();
    }

    fn parse_slot_from_claude_title(&self, title: &str) -> Option<u8> {
        // Format: "claude-{slot}-{project}" e.g., "claude-2-treatmentplan"
        let parts: Vec<&str> = title.split('-').collect();
        if parts.len() >= 2 {
            parts[1].parse().ok()
        } else {
            None
        }
    }

    fn assign_tool_panes(&mut self) {
        // Tool panes are named: lazygit, bv, test, adhoc (no slot suffix)
        // We need to figure out which tools belong to which slot
        // Strategy: Group panes by their approximate position (will refine later)

        // For MVP: Collect all tool pane IDs
        let tool_names = ["lazygit", "bv", "test", "adhoc"];
        let mut tool_panes: Vec<PaneId> = Vec::new();

        for name in tool_names {
            // There might be multiple panes with same name
            for (title, &pane_id) in &self.pane_ids {
                if title == name {
                    tool_panes.push(pane_id);
                }
            }
        }

        // Distribute tools evenly across slots that have claude panes
        // This is a simplification - proper implementation would use pane positions
        let slots_with_claude: Vec<u8> = self.slots.keys().copied().collect();
        let tools_per_slot = if slots_with_claude.is_empty() {
            0
        } else {
            tool_panes.len() / slots_with_claude.len()
        };

        let mut tool_iter = tool_panes.into_iter();
        for slot in slots_with_claude {
            if let Some(slot_panes) = self.slots.get_mut(&slot) {
                for _ in 0..tools_per_slot.min(4) {
                    if let Some(tool_id) = tool_iter.next() {
                        slot_panes.tools.push(tool_id);
                    }
                }
            }
        }
    }

    fn handle_command(&mut self, command: &str) {
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        match parts.as_slice() {
            ["grid"] => {
                self.mode = Mode::Grid;
                self.apply_mode();
            }
            ["focus", slot_str] => {
                if let Ok(slot) = slot_str.parse::<u8>() {
                    if (1..=8).contains(&slot) {
                        self.mode = Mode::Focus(slot);
                        self.apply_mode();
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_mode(&self) {
        match &self.mode {
            Mode::Grid => self.apply_grid_mode(),
            Mode::Focus(slot) => self.apply_focus_mode(*slot),
        }
    }

    fn apply_grid_mode(&self) {
        // Show all claude panes, hide all tool panes
        for slot_panes in self.slots.values() {
            if let Some(claude_id) = slot_panes.claude {
                show_pane_with_id(claude_id, false, false);
            }
            for &tool_id in &slot_panes.tools {
                hide_pane_with_id(tool_id);
            }
        }
    }

    fn apply_focus_mode(&self, focus_slot: u8) {
        for (&slot, slot_panes) in &self.slots {
            if slot == focus_slot {
                // Show this slot's claude and tools
                if let Some(claude_id) = slot_panes.claude {
                    show_pane_with_id(claude_id, false, true);
                }
                for &tool_id in &slot_panes.tools {
                    show_pane_with_id(tool_id, false, false);
                }
            } else {
                // Hide other slots
                if let Some(claude_id) = slot_panes.claude {
                    hide_pane_with_id(claude_id);
                }
                for &tool_id in &slot_panes.tools {
                    hide_pane_with_id(tool_id);
                }
            }
        }
    }
}
```

**Step 2: Verify it compiles**

```bash
cd <repo>
rustup target add wasm32-wasip1 2>/dev/null || rustup target add wasm32-wasi
cargo build --target wasm32-wasip1 2>/dev/null || cargo build --target wasm32-wasi
```

Expected: Build succeeds (warnings OK)

**Step 3: Commit**

```bash
git add -A
git commit -m "Add pane tracking and mode switching logic"
```

---

### Task 3: Create Layout Generator Script

**Files:**
- Create: `<repo>/scripts/generate-layout.sh`

**Step 1: Create scripts directory and generator**

```bash
mkdir -p <repo>/scripts
```

**Step 2: Write the generator script**

```bash
#!/bin/bash
# Generates workspace.kdl from projects.json

set -e

PROJECTS_JSON="$HOME/.config/zellij/projects.json"
OUTPUT_FILE="$HOME/.config/zellij/layouts/workspace.kdl"

if [ ! -f "$PROJECTS_JSON" ]; then
    echo "Error: $PROJECTS_JSON not found"
    exit 1
fi

# Start the layout file
cat > "$OUTPUT_FILE" << 'HEADER'
// Generated by workspace-switcher/scripts/generate-layout.sh
// Regenerate with: ./scripts/generate-layout.sh

layout {
    tab name="workspace" focus=true {
HEADER

# Generate panes for each slot 1-8
for slot in {1..8}; do
    # Get project name and path from projects.json using jq
    project_name=$(jq -r ".slots[\"$slot\"] // \"empty\"" "$PROJECTS_JSON")

    if [ "$project_name" != "null" ] && [ "$project_name" != "empty" ]; then
        project_path=$(jq -r ".projects[] | select(.name == \"$project_name\") | .path" "$PROJECTS_JSON")
    else
        project_name="empty"
        project_path=""
    fi

    # Write pane group for this slot
    cat >> "$OUTPUT_FILE" << PANES
        // Slot $slot: $project_name
        pane split_direction="horizontal" {
PANES

    if [ -n "$project_path" ] && [ "$project_path" != "null" ]; then
        cat >> "$OUTPUT_FILE" << PANES
            pane size="50%" name="claude-$slot-$project_name" cwd="$project_path"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="lazygit" cwd="$project_path" command="lazygit"
                    pane name="bv" cwd="$project_path" command="bv"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test" cwd="$project_path"
                    pane name="adhoc" cwd="$project_path"
                }
            }
        }

PANES
    else
        cat >> "$OUTPUT_FILE" << PANES
            pane size="50%" name="claude-$slot-empty"
            pane size="50%" split_direction="vertical" {
                pane size="50%" split_direction="horizontal" {
                    pane name="lazygit"
                    pane name="bv"
                }
                pane size="50%" split_direction="horizontal" {
                    pane name="test"
                    pane name="adhoc"
                }
            }
        }

PANES
    fi
done

# Close the layout
cat >> "$OUTPUT_FILE" << 'FOOTER'
    }
}
FOOTER

echo "Generated: $OUTPUT_FILE"
```

**Step 3: Make it executable**

```bash
chmod +x <repo>/scripts/generate-layout.sh
```

**Step 4: Test the generator**

```bash
<repo>/scripts/generate-layout.sh
cat ~/.config/zellij/layouts/workspace.kdl | head -50
```

Expected: Layout file generated with slot panes

**Step 5: Commit**

```bash
cd <repo>
git add -A
git commit -m "Add layout generator script"
```

---

### Task 4: Build and Install Plugin

**Files:**
- Modify: `<repo>/Cargo.toml` (if needed)

**Step 1: Build release WASM**

```bash
cd <repo>
cargo build --release --target wasm32-wasip1 2>/dev/null || cargo build --release --target wasm32-wasi
```

Expected: Creates `target/wasm32-wasi*/release/workspace_switcher.wasm`

**Step 2: Copy to Zellij plugins directory**

```bash
mkdir -p ~/.config/zellij/plugins
cp target/wasm32-wasi*/release/workspace_switcher.wasm ~/.config/zellij/plugins/
ls -la ~/.config/zellij/plugins/workspace_switcher.wasm
```

Expected: File exists, reasonable size (~100KB-1MB)

**Step 3: Commit build artifacts note**

```bash
git add -A
git commit -m "Document build process" --allow-empty
```

---

### Task 5: Add Keybindings to Zellij Config

**Files:**
- Modify: `~/.config/zellij/config.kdl`

**Step 1: Read current config**

```bash
cat ~/.config/zellij/config.kdl
```

**Step 2: Add keybindings for workspace switching**

Add this block inside the `keybinds` section (create if missing):

```kdl
keybinds {
    shared {
        bind "Alt 0" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "grid";
            };
        }
        bind "Alt 1" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 1";
            };
        }
        bind "Alt 2" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 2";
            };
        }
        bind "Alt 3" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 3";
            };
        }
        bind "Alt 4" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 4";
            };
        }
        bind "Alt 5" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 5";
            };
        }
        bind "Alt 6" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 6";
            };
        }
        bind "Alt 7" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 7";
            };
        }
        bind "Alt 8" {
            MessagePlugin "file:~/.config/zellij/plugins/workspace_switcher.wasm" {
                payload "focus 8";
            };
        }
    }
}
```

**Step 3: Validate config syntax**

```bash
~/.local/bin/zellij setup --check
```

Expected: No errors (warnings OK)

---

### Task 6: Update Layout to Load Plugin

**Files:**
- Modify: `~/.config/zellij/layouts/workspace.kdl`

**Step 1: Add plugin pane to layout**

The plugin needs to be loaded. Add at the start of the tab, before other panes:

```kdl
layout {
    tab name="workspace" focus=true {
        // Load the workspace switcher plugin (hidden)
        pane size=1 borderless=true {
            plugin location="file:~/.config/zellij/plugins/workspace_switcher.wasm"
        }

        // ... rest of panes ...
    }
}
```

Alternatively, regenerate the layout with the script and manually add the plugin pane.

---

### Task 7: Test the Plugin

**Step 1: Kill any existing session**

```bash
~/.local/bin/zellij delete-session workspace --force 2>/dev/null || true
```

**Step 2: Launch with new layout**

```bash
~/.local/bin/zellij --layout ~/.config/zellij/layouts/workspace.kdl -s workspace
```

**Step 3: Test keybindings**

1. Press `Alt+0` - Should show grid view (8 Claude panes)
2. Press `Alt+1` - Should show focus view for slot 1
3. Press `Alt+2` - Should show focus view for slot 2
4. Press `Alt+0` - Should return to grid view

**Step 4: Verify pane hiding**

When in focus mode, other Claude panes should be completely invisible (not stacked, not minimized).

---

### Task 8: Fix Tool Pane Assignment

The MVP assigns tools naively. This task improves it.

**Files:**
- Modify: `<repo>/src/main.rs`

**Step 1: Improve tool pane assignment using position**

Replace `assign_tool_panes` method with position-based logic:

```rust
fn assign_tool_panes(&mut self, manifest: &PaneManifest) {
    // Tool panes are named: lazygit, bv, test, adhoc
    // We assign them to slots based on their spatial proximity to claude panes

    let tool_names = ["lazygit", "bv", "test", "adhoc"];

    for (_tab_index, panes) in &manifest.panes {
        // Find claude panes and their positions
        let mut claude_positions: Vec<(u8, i32, i32)> = Vec::new(); // (slot, x, y)

        for pane in panes {
            if pane.title.starts_with("claude-") {
                if let Some(slot) = self.parse_slot_from_claude_title(&pane.title) {
                    claude_positions.push((slot, pane.pane_x as i32, pane.pane_y as i32));
                }
            }
        }

        // For each tool pane, find nearest claude pane
        for pane in panes {
            if tool_names.contains(&pane.title.as_str()) {
                let pane_id = if pane.is_plugin {
                    PaneId::Plugin(pane.id)
                } else {
                    PaneId::Terminal(pane.id)
                };

                // Find closest claude pane by position
                let mut closest_slot = 1u8;
                let mut min_distance = i32::MAX;

                for &(slot, cx, cy) in &claude_positions {
                    let dx = (pane.pane_x as i32) - cx;
                    let dy = (pane.pane_y as i32) - cy;
                    let distance = dx.abs() + dy.abs();
                    if distance < min_distance {
                        min_distance = distance;
                        closest_slot = slot;
                    }
                }

                if let Some(slot_panes) = self.slots.get_mut(&closest_slot) {
                    if slot_panes.tools.len() < 4 {
                        slot_panes.tools.push(pane_id);
                    }
                }
            }
        }
    }
}
```

**Step 2: Update `update_pane_map` to pass manifest**

```rust
fn update_pane_map(&mut self, manifest: PaneManifest) {
    self.pane_ids.clear();
    self.slots.clear();

    for (_tab_index, panes) in &manifest.panes {
        for pane in panes {
            let pane_id = if pane.is_plugin {
                PaneId::Plugin(pane.id)
            } else {
                PaneId::Terminal(pane.id)
            };

            self.pane_ids.insert(pane.title.clone(), pane_id);

            if pane.title.starts_with("claude-") {
                if let Some(slot) = self.parse_slot_from_claude_title(&pane.title) {
                    let slot_panes = self.slots.entry(slot).or_default();
                    slot_panes.claude = Some(pane_id);
                }
            }
        }
    }

    self.assign_tool_panes(&manifest);
}
```

**Step 3: Rebuild and reinstall**

```bash
cd <repo>
cargo build --release --target wasm32-wasip1 || cargo build --release --target wasm32-wasi
cp target/wasm32-wasi*/release/workspace_switcher.wasm ~/.config/zellij/plugins/
```

**Step 4: Commit**

```bash
git add -A
git commit -m "Improve tool pane assignment using spatial proximity"
```

---

## Verification Checklist

- [ ] Plugin compiles to WASM
- [ ] Plugin installed to `~/.config/zellij/plugins/`
- [ ] Layout generated from `projects.json`
- [ ] Keybindings added to config
- [ ] `Alt+0` switches to grid mode (8 Claudes visible)
- [ ] `Alt+1-8` switches to focus mode (1 Claude + 4 tools)
- [ ] Hidden panes are truly invisible
- [ ] Pane state persists across mode switches
