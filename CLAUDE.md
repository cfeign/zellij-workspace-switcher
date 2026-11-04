# CLAUDE.md

Guidance for Claude Code when working in this repository. Read `docs/architecture.md` before touching `src/main.rs`.

## What this is

A Zellij WASM plugin plus bash scripts that run 6 or 8 project "slots", each with five floating panes (claude, git, issues, test, adhoc), and switch between a grid of all Claude panes (Alt+0) and a single slot's full view (Alt+1-8). README.md has the user-facing picture; `docs/architecture.md` has the mechanics; `docs/customizing.md` has every knob.

## Build and install

```bash
cargo build --release --target wasm32-wasip1          # needs: rustup target add wasm32-wasip1
cp target/wasm32-wasip1/release/workspace-switcher.wasm \
   ~/.config/zellij/plugins/workspace_switcher.wasm    # note: hyphen in cargo output, underscore installed
./scripts/generate-layout.sh                           # after editing projects.json; 6 or 8 as arg
./setup.sh                                             # first time: links, builds, generates
```

After rebuilding the WASM, restart the session (`zellij-workspace nuke && zellij-workspace start`); Zellij caches plugins. There are no automated tests. The plugin is verified by running it; logs are `eprintln!` lines prefixed `[ws]` in Zellij's stderr.

Shell scripts: `bash -n scripts/*` for syntax. `generate-layout.sh` can be run against a scratch config with `WORKSPACE_PROJECTS_JSON=/path/to/test.json HOME=/tmp/x`.

## Invariants to keep

- **Pane naming convention** `claude-<slot>-<project>`, `git-<slot>`, `issues-<slot>`, `test-<slot>`, `adhoc-<slot>`. `generate-layout.sh` writes it, `PaneType::prefix()` in main.rs parses it, `zellij-workspace` greps process names derived from it. Change all three together.
- **No hardcoded machine paths.** Commands reach the plugin through the layout's plugin configuration block (`claude_cmd`, `git_cmd`, ...), written by `generate-layout.sh` after resolving names on PATH. Scripts find each other through `readlink` on their own symlink, and expand `~` from projects.json.
- **projects.json is the single source of truth** for grid size, shell, pane commands, projects, slots. Scripts read it; the picker writes it; the layout is derived from it.
- Scripts run under Zellij's minimal environment. Keep the `export PATH=...` prelude at the top of each.

## Where things live

| Change | File |
|---|---|
| Keybindings, theme, Zellij options | `config/config.kdl` |
| Pane commands, grid size, projects | `~/.config/zellij/projects.json` (example in `config/`) |
| Grid geometry, focus geometry, pane types, swap logic | `src/main.rs` |
| Layout generation, command resolution | `scripts/generate-layout.sh` |
| Session start/attach/cleanup | `scripts/zellij-workspace` |
| Picker UI, sound and font pickers | `scripts/zellij-swap` |
| How Claude Code is launched in a pane | `scripts/claude-workspace` |
| Attention-highlight hook | `scripts/workspace-claude-hook` |
| Ghostty settings | `config/ghostty/config` |

## Gotchas

- Zellij titles a newly opened command pane after its command. The plugin's `pending_renames` queue matches on the command basename to re-title it. If you change what a pane runs, the match still works because the basename comes from the same config.
- `quick-focus` numbering in focus mode is 1 claude, 2 git, 3 issues, 4 test, 5 adhoc (the private version skipped 4).
- macOS terminals must send Option as Alt or none of the bindings fire. This is a terminal setting, not a Zellij one.
- `docs/history/` is the original author's design trail. Pane names in there are the old ones; see `docs/history/README.md` for the mapping.
