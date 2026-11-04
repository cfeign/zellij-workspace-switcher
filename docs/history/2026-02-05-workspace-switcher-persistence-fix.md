# Workspace Switcher Persistence Fix

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix workspace_switcher plugin so all 8 slots are detected and displayed correctly on both fresh session creates and session reattach, enabling true persistent Zellij sessions.

**Architecture:** Three bugs to fix: (1) plugin has no initial mode — it never shows panes on startup, relying on `hide_floating_panes=true` and hoping the user hits Alt+0 before seeing anything, (2) `all_panes` HashMap accumulates stale entries and should do a full rebuild from the manifest each PaneUpdate, (3) add enhanced debug logging so we can diagnose slot detection issues. The launcher changes (zellij-dev2, Ghostty Zellij 2.app) are already working.

**Tech Stack:** Rust, zellij-tile 0.43.1, WASM target

**Safety:** The original `Ghostty Zellij.app` and `zellij-dev` are untouched and remain as fallback. All changes are to the plugin source and `workspace2` session only.

---

### Task 1: Add enhanced debug logging to diagnose slot detection

**Files:**
- Modify: `<repo>/src/main.rs:90-106`

**Step 1: Update `update_pane_map` to log all panes received and do a full rebuild**

Replace lines 90-106 with:

```rust
fn update_pane_map(&mut self, manifest: &PaneManifest) {
    // Full rebuild from manifest each time (not accumulate)
    self.all_panes.clear();

    let mut total_panes = 0;
    for (_tab_index, panes) in &manifest.panes {
        for pane in panes {
            total_panes += 1;
            let pane_id = if pane.is_plugin {
                PaneId::Plugin(pane.id)
            } else {
                PaneId::Terminal(pane.id)
            };

            // Log every pane we see on first load or when count changes
            eprintln!("[ws] Pane: '{}' id={:?} plugin={} floating={}",
                pane.title, pane_id, pane.is_plugin, pane.is_floating);

            self.all_panes.insert(pane.title.clone(), pane_id);
        }
    }
    eprintln!("[ws] Total panes in manifest: {}, in map: {}", total_panes, self.all_panes.len());
    self.rebuild_slots();
}
```

**Step 2: Build and deploy to check logs**

Run:
```bash
cd <repo>
cargo build --target wasm32-wasip1 --release 2>&1
cp target/wasm32-wasip1/release/workspace_switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
```

**Step 3: Kill workspace2 and relaunch to check logs**

Run:
```bash
~/.local/bin/zellij kill-session workspace2 2>/dev/null; sleep 1
# Then launch "Ghostty Zellij 2" app
# Check plugin logs with: grep '\[ws\]' /tmp/zellij-*/zellij-log/*.log
```

Expected: logs should show all 40 panes (8 claude + 32 tool) being detected. If not, the log will tell us exactly which are missing and why.

**Step 4: Commit**

```bash
cd <repo>
git add src/main.rs
git commit -m "$(cat <<'EOF'
fix: rebuild pane map from manifest instead of accumulating

Clears all_panes on each PaneUpdate to prevent stale entries.
Adds detailed debug logging to diagnose slot detection issues.
EOF
)"
```

---

### Task 2: Auto-apply grid mode on startup

The plugin starts with `current_mode: None` and never applies a mode until the user presses Alt+0 or Alt+1-8. With `hide_floating_panes=true` in the layout, this means panes may be in an undefined visibility state on startup.

**Files:**
- Modify: `<repo>/src/main.rs`

**Step 1: Add a `panes_ready` flag and auto-apply grid mode once all slots are detected**

Add field to State struct:

```rust
#[derive(Default)]
struct State {
    all_panes: HashMap<String, PaneId>,
    slots: HashMap<u8, SlotPanes>,
    current_mode: Option<Mode>,
    /// Whether we've auto-applied initial mode
    initialized: bool,
}
```

**Step 2: Add auto-init logic at the end of `update_pane_map`**

After the `self.rebuild_slots()` call, add:

```rust
// Auto-apply grid mode once we detect enough slots
if !self.initialized && self.slots.len() >= 2 {
    eprintln!("[ws] Auto-initializing grid mode with {} slots", self.slots.len());
    self.apply_grid_mode();
    self.initialized = true;
}
```

Note: We use `>= 2` rather than `== 8` because some slots (like slot 8 with no claude command) might not be detected. We want grid mode applied as soon as panes are available, and subsequent PaneUpdate events will add more slots as they arrive — the user can press Alt+0 again to refresh if needed.

**Step 3: Build and deploy**

Run:
```bash
cd <repo>
cargo build --target wasm32-wasip1 --release 2>&1
cp target/wasm32-wasip1/release/workspace_switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
```

**Step 4: Test fresh session**

Run:
```bash
~/.local/bin/zellij kill-session workspace2 2>/dev/null; sleep 1
```
Then launch "Ghostty Zellij 2" app. Expected: grid mode auto-applies, showing all 8 claude panes in 2x4 grid.

**Step 5: Test reattach**

Close Ghostty (Cmd+Q). Reopen "Ghostty Zellij 2". Expected: reattaches to running session, grid (or last mode) is preserved. No "Waiting to run" prompts.

**Step 6: Commit**

```bash
cd <repo>
git add src/main.rs
git commit -m "$(cat <<'EOF'
feat: auto-apply grid mode on startup

Plugin now auto-initializes to grid mode once it detects enough
slots, so panes are visible immediately without needing Alt+0.
EOF
)"
```

---

### Task 3: Add a "rescan" command for manual recovery

If auto-init happens before all panes are ready, the user needs a way to force a re-scan and re-apply without killing the session.

**Files:**
- Modify: `<repo>/src/main.rs` (handle_command)
- Modify: `~/.config/zellij/config.kdl` (add keybinding)

**Step 1: Add "rescan" command to handle_command**

Add a new match arm in `handle_command`:

```rust
["rescan"] => {
    eprintln!("[ws] Rescan requested - resetting state");
    self.initialized = false;
    self.current_mode = None;
    // Next PaneUpdate will trigger auto-init
    // Force a grid mode apply now with current data
    self.apply_grid_mode();
    self.initialized = true;
}
```

**Step 2: Add keybinding in config.kdl**

Add to the `shared` keybinds section (near the other workspace_switcher bindings):

```kdl
bind "Alt 9" { MessagePlugin "workspace_switcher" { payload "rescan"; }; }
```

**Step 3: Build, deploy, test**

Run:
```bash
cd <repo>
cargo build --target wasm32-wasip1 --release 2>&1
cp target/wasm32-wasip1/release/workspace_switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
```

Test: In a running workspace2 session, press Alt+9. All 8 claude panes should appear in grid layout.

**Step 4: Commit**

```bash
cd <repo>
git add src/main.rs
git commit -m "$(cat <<'EOF'
feat: add rescan command (Alt+9) for manual recovery

Allows forcing a full pane re-scan and grid mode re-apply
without needing to kill the session.
EOF
)"
```

---

### Task 4: Reduce verbose logging for production

After confirming everything works, dial back the per-pane logging to only log when slot counts change.

**Files:**
- Modify: `<repo>/src/main.rs`

**Step 1: Replace per-pane logging with summary logging**

In `update_pane_map`, replace the per-pane eprintln with:

```rust
fn update_pane_map(&mut self, manifest: &PaneManifest) {
    self.all_panes.clear();

    for (_tab_index, panes) in &manifest.panes {
        for pane in panes {
            let pane_id = if pane.is_plugin {
                PaneId::Plugin(pane.id)
            } else {
                PaneId::Terminal(pane.id)
            };
            self.all_panes.insert(pane.title.clone(), pane_id);
        }
    }

    let old_slot_count = self.slots.len();
    self.rebuild_slots();
    if self.slots.len() != old_slot_count {
        eprintln!("[ws] Slots updated: {} -> {} ({:?})",
            old_slot_count, self.slots.len(), self.slots.keys().collect::<Vec<_>>());
    }
}
```

**Step 2: Build and deploy**

Run:
```bash
cd <repo>
cargo build --target wasm32-wasip1 --release 2>&1
cp target/wasm32-wasip1/release/workspace_switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
```

**Step 3: Commit**

```bash
cd <repo>
git add src/main.rs
git commit -m "$(cat <<'EOF'
chore: reduce plugin logging verbosity

Only log when slot count changes instead of every pane on every update.
EOF
)"
```

---

### Task 5: Final integration test

**Step 1: Fresh session test**

```bash
~/.local/bin/zellij kill-session workspace2 2>/dev/null
~/.local/bin/zellij delete-all-sessions --yes 2>/dev/null
```
Launch "Ghostty Zellij 2". Verify:
- [ ] Grid mode auto-applies (8 claude panes visible)
- [ ] Alt+1 switches to focus mode (claude + 4 tools for slot 1)
- [ ] Alt+0 switches back to grid
- [ ] Alt+5 shows slot 5 focus view
- [ ] Alt+9 rescans and re-applies grid

**Step 2: Persistence test**

With session running:
- [ ] Close Ghostty (Cmd+Q)
- [ ] Run `zellij list-sessions` — workspace2 should show as active
- [ ] Reopen "Ghostty Zellij 2" — reattaches, no "Waiting to run" prompts
- [ ] All panes have their running processes (claude, lazygit, etc.)
- [ ] Alt+0/1-8 switching works

**Step 3: Verify fallback still works**

- [ ] Launch original "Ghostty Zellij" app — still works as before with its own session

**Step 4: Commit final state**

```bash
cd <repo>
git add -A
git commit -m "$(cat <<'EOF'
feat: workspace switcher v3 - persistent session support

- Full pane map rebuild from manifest (no stale accumulation)
- Auto-apply grid mode on startup
- Alt+9 rescan for manual recovery
- Reduced logging verbosity
EOF
)"
```
