# Smart Alt+u: zellij-urlpick Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inline urlscan keybinding with a smart wrapper script that opens urlscan when URLs are present, or copies the last Claude turn to clipboard when no URLs are found.

**Architecture:** Single bash script (`scripts/zellij-urlpick`) called by `Alt+u` via Zellij `DumpScreen` + `Run`. Script reads the dump file, branches on URL presence, and either `exec`s urlscan or extracts the last Claude turn via `❯` prompt markers and pipes to `pbcopy`.

**Tech Stack:** Bash, urlscan, pbcopy, Zellij KDL config

**Spec:** `docs/superpowers/specs/2026-03-12-smart-urlpick-design.md`

---

## Chunk 1: The Script

### Task 1: Create `scripts/zellij-urlpick` with URL detection and branching

**Files:**
- Create: `scripts/zellij-urlpick`

- [ ] **Step 1: Create the script with shebang, PATH, and dump file setup**

```bash
#!/usr/bin/env bash
set -euo pipefail

# Ensure homebrew and local binaries are on PATH (bash launched by Zellij may not have it)
export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"

DUMP_FILE="/tmp/zellij-urlscan.txt"

# Wait for DumpScreen to flush (not sequenced with Run)
sleep 0.1

if [[ ! -f "$DUMP_FILE" ]]; then
    echo "No screen dump found"
    sleep 1
    exit 0
fi
```

- [ ] **Step 2: Add URL detection branch — exec urlscan if URLs found**

Append after the dump file check:

```bash
# Branch A: URLs found — open urlscan
if grep -qE 'https?://' "$DUMP_FILE"; then
    if ! command -v urlscan &>/dev/null; then
        echo "urlscan not found. Install with: uv tool install urlscan"
        sleep 2
        exit 1
    fi
    exec urlscan "$DUMP_FILE"
fi
```

- [ ] **Step 3: Add Claude turn extraction — Branch B**

Append after the URL branch:

```bash
# Branch B: No URLs — try to extract last Claude turn
# Claude Code uses ❯ as the user input prompt
# Extract everything between the last two ❯ lines (the last Claude response)
last_turn=$(awk '
    /❯/ { prompts[++n] = NR }
    { lines[NR] = $0 }
    END {
        if (n < 2) exit 1
        start = prompts[n-1] + 1
        end = prompts[n] - 1
        for (i = start; i <= end; i++) print lines[i]
    }
' "$DUMP_FILE" | sed '/^[[:space:]]*$/{ 1d; $d; }')

if [[ -n "$last_turn" ]]; then
    printf '%s' "$last_turn" | pbcopy
    echo "Copied to clipboard"
    sleep 1
    exit 0
fi

# Branch C: No URLs, no Claude turn
echo "No URLs or Claude output found"
sleep 1
exit 0
```

- [ ] **Step 4: Make the script executable**

Run: `chmod +x scripts/zellij-urlpick`

- [ ] **Step 5: Verify the complete script reads correctly**

Run: `cat scripts/zellij-urlpick`

Expected: Complete script with shebang, PATH export, dump file wait, Branch A (urlscan), Branch B (Claude turn → pbcopy), Branch C (fallback message).

- [ ] **Step 6: Commit**

```bash
git add scripts/zellij-urlpick
git commit -m "feat: add zellij-urlpick smart URL/clipboard script"
```

---

### Task 2: Update `config/config.kdl` keybinding

**Files:**
- Modify: `config/config.kdl:32-39`

- [ ] **Step 1: Replace the inline urlscan binding with the script call**

Change the existing `Alt u` binding (lines 32-39) from:

```kdl
// URL picker — dump screen first, then open urlscan in floating pane
bind "Alt u" {
    DumpScreen "/tmp/zellij-urlscan.txt";
    Run "bash" "-c" "~/.local/bin/urlscan /tmp/zellij-urlscan.txt" {
        floating true
        close_on_exit true
    }
}
```

To:

```kdl
// Smart picker — URLs → urlscan, no URLs → copy last Claude turn
bind "Alt u" {
    DumpScreen "/tmp/zellij-urlscan.txt";
    Run "bash" "-c" "~/.local/bin/zellij-urlpick" {
        floating true
        close_on_exit true
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add config/config.kdl
git commit -m "feat: update Alt+u to use smart zellij-urlpick script"
```

---

### Task 3: Add `zellij-urlpick` to `setup.sh` symlink loop

**Files:**
- Modify: `setup.sh:12`

- [ ] **Step 1: Add `zellij-urlpick` to the `for script in ...` list**

Change line 12 from:

```bash
for script in zellij-swap zellij-workspace zellij-dev zellij-dev2 claude-workspace dev-tab; do
```

To:

```bash
for script in zellij-swap zellij-workspace zellij-dev zellij-dev2 claude-workspace dev-tab zellij-urlpick; do
```

- [ ] **Step 2: Create the symlink manually now (so it works before next setup.sh run)**

Run: `ln -sf ~/code/workspace-switcher/scripts/zellij-urlpick ~/.local/bin/zellij-urlpick`

- [ ] **Step 3: Verify symlink**

Run: `ls -la ~/.local/bin/zellij-urlpick`

Expected: symlink pointing to `~/code/workspace-switcher/scripts/zellij-urlpick`

- [ ] **Step 4: Commit**

```bash
git add setup.sh
git commit -m "feat: add zellij-urlpick to setup.sh symlink loop"
```

---

## Chunk 2: Manual Testing

### Task 4: Test all three branches

No automated tests — this is an integration-tested runtime tool (same pattern as all other scripts in this repo per CLAUDE.md).

- [ ] **Step 1: Restart Zellij session to pick up config changes**

- [ ] **Step 2: Test Branch A (URLs) — in a Claude pane with a URL visible**

1. Make sure a URL like `https://example.com` is visible on screen
2. Press `Alt+u`
3. Expected: urlscan opens in floating pane showing the URL with context
4. Press `Esc` to close

- [ ] **Step 3: Test Branch B (Claude turn, no URLs) — in a Claude pane with a response but no URLs**

1. Have a Claude response visible that contains no URLs
2. Press `Alt+u`
3. Expected: floating pane flashes "Copied to clipboard", auto-closes after 1s
4. Paste (`Cmd+v`) somewhere to verify the Claude response was captured

- [ ] **Step 4: Test Branch C (no URLs, no Claude output) — in a non-Claude pane**

1. Focus a lazygit or shell pane
2. Press `Alt+u`
3. Expected: floating pane flashes "No URLs or Claude output found", auto-closes after 1s
