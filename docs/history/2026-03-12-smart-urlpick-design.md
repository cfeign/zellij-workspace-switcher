# Smart Alt+u: zellij-urlpick

## Summary

Replace the current inline `Alt+u` urlscan binding with a wrapper script (`zellij-urlpick`) that branches based on screen content:
- **URLs present** → open urlscan picker
- **No URLs, Claude turn found** → copy last Claude Code turn to clipboard
- **No URLs, no Claude turn** → show "No URLs or Claude output found", exit

Single keybinding, context-aware behavior.

## Trigger

`Alt+u` in Zellij (all modes except locked).

## Flow

1. Zellij `DumpScreen` captures the focused pane to `/tmp/zellij-urlscan.txt`
2. Floating pane opens running `zellij-urlpick`
3. Script waits briefly (`sleep 0.1`) for dump file to flush (DumpScreen and Run are not sequenced)
4. Script greps dump for `https?://` URLs
5. **Branch A — URLs found:** `exec urlscan /tmp/zellij-urlscan.txt`. User picks a URL with arrow keys + Enter (opens in browser). Esc/q/Q to quit. Floating pane closes on exit.
6. **Branch B — Claude turn found, no URLs:** Extract last Claude turn, pipe to `pbcopy`, print "Copied to clipboard", sleep 1s, exit. Floating pane auto-closes.
7. **Branch C — No URLs, no Claude turn:** Print "No URLs or Claude output found", sleep 1s, exit.

## Claude Turn Extraction (Branch B)

Claude Code terminal output uses these markers:
- `❯` — user input prompt
- `●` — assistant response lines

`DumpScreen` produces plain text (no ANSI escapes) on current Zellij versions. If this changes, add a strip step.

To extract the last turn:
1. Scan the dump bottom-to-top
2. Find the last `❯` line (current empty prompt)
3. Continue scanning upward to find the previous `❯` line (user input that triggered the response)
4. Capture everything between those two `❯` lines, excluding the user input line itself
5. Strip leading/trailing blank lines
6. Pipe result to `pbcopy`

**Viewport limitation:** `DumpScreen` captures only the visible viewport. Long Claude responses will be truncated to what's on screen. This is acceptable — the user can scroll to see more before pressing Alt+u.

## Files

| File | Change |
|------|--------|
| `scripts/zellij-urlpick` | New script — branching logic, Claude turn extraction |
| `config/config.kdl` | Update `Alt u` binding to call `zellij-urlpick` |
| `setup.sh` | Add `zellij-urlpick` to symlink loop (add to `for script in ...` list) |

## Script Conventions

- `export PATH="/opt/homebrew/bin:$HOME/.local/bin:$PATH"` at top (matching other scripts)
- Check `urlscan` exists before `exec` (print error + sleep if missing)

## Dependencies

- `urlscan` (already installed via `uv tool install urlscan`)
- `pbcopy` (macOS built-in)
- `grep` (for URL detection)

## Keybinding (config.kdl)

```kdl
bind "Alt u" {
    DumpScreen "/tmp/zellij-urlscan.txt";
    Run "bash" "-c" "~/.local/bin/zellij-urlpick" {
        floating true
        close_on_exit true
    }
}
```
