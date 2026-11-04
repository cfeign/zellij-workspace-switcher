# zellij-workspace-switcher

Run up to 8 projects at once in one terminal, each with its own Claude Code session and tool panes, and flip between an overview grid and a single-project focus view with one key.

```
 Grid mode (Alt+0)                       Focus mode (Alt+3)
┌──────┬──────┬──────┬──────┐           ┌─────────────┬──────┬──────┐
│ cl 1 │ cl 2 │ cl 3 │ cl 4 │           │             │ git  │issues│
├──────┼──────┼──────┼──────┤           │  claude 3   ├──────┼──────┤
│ cl 5 │ cl 6 │ cl 7 │ cl 8 │           │             │ test │adhoc │
└──────┴──────┴──────┴──────┘           └─────────────┴──────┴──────┘
```

Each **slot** holds one project and five floating panes: Claude Code, a git pane, an issues pane, a test shell, and a scratch shell. A small Zellij WASM plugin shows, hides, and repositions them. Bash scripts handle the session lifecycle and a fuzzy project picker. Grid size is 6 (2x3) or 8 (2x4).

Built for macOS with Ghostty and fish, but nothing requires them: the shell and every pane command are configurable, and Zellij keybindings work in any terminal that passes Alt through.

## Install

Dependencies: [zellij](https://zellij.dev), [jq](https://jqlang.github.io/jq), [fzf](https://github.com/junegunn/fzf), [Rust](https://rustup.rs) (to build the plugin), and [Claude Code](https://claude.com/claude-code).

```sh
brew install zellij jq fzf
git clone https://github.com/cfeign/zellij-workspace-switcher ~/code/zellij-workspace-switcher
cd ~/code/zellij-workspace-switcher
./setup.sh
```

`setup.sh` links the scripts into `~/.local/bin`, links `config/config.kdl` to `~/.config/zellij/config.kdl` (backing up any existing one), builds and installs the plugin, creates `~/.config/zellij/projects.json` from the example, and generates both layouts.

Then:

1. Edit `~/.config/zellij/projects.json` and run `generate-layout.sh`.
2. **macOS:** make your terminal send Option as Alt. In Ghostty that is `macos-option-as-alt = true` in `~/.config/ghostty/config`. Without it every Alt binding is dead. See [config/ghostty/config](config/ghostty/config).
3. `zellij-workspace start`

## projects.json

```json
{
  "grid": 8,
  "projects_root": "~/code",
  "shell": "fish",
  "panes": {
    "claude": "claude-workspace",
    "git": "lazygit",
    "issues": "",
    "test": "",
    "adhoc": ""
  },
  "projects": [
    { "name": "my-app", "path": "~/code/my-app" }
  ],
  "slots": { "1": "my-app" }
}
```

| Key | Meaning |
|---|---|
| `grid` | 6 or 8. Default for `zellij-workspace start`; override with `--grid`. |
| `projects_root` | Where the picker looks for unregistered projects, and where `N`ew projects are created. |
| `shell` | Shell for empty pane commands. Falls back to fish, then zsh, then bash. |
| `panes` | Command for each pane type. Empty string means the shell. A bare name is looked up on PATH; `~` and absolute paths work. |
| `projects` | Name and path of every known project. |
| `slots` | Which project sits in which slot. Empty slots get panes with no cwd. |

Run `scripts/generate-layout.sh` after editing. The picker (Alt+s) edits this file for you and regenerates automatically.

## Keys

| Key | Action |
|---|---|
| `Alt+0` | Grid mode: every slot's Claude pane, tools hidden |
| `Alt+1`..`8` | Focus mode for that slot: Claude left, four tools right |
| `Alt+Shift+1`..`8` | In grid: jump to slot N without leaving grid. In focus: jump to pane 1 claude, 2 git, 3 issues, 4 test, 5 adhoc |
| `Alt+s` | Project picker (see below) |
| `Alt+f` | Fullscreen the focused pane |
| `Alt+h/j/k/l` | Move focus between panes |
| `Alt+u` | Copy Claude's last reply to the clipboard and open any URLs in it |
| `Alt+9` | Rescan: reset the plugin if panes get out of sync |
| `Ctrl+g` | Lock mode on/off (swallows all bindings, for apps that need Alt) |
| `Ctrl+p` | Resize mode (`hjkl`, `Esc` to leave) |
| `Ctrl+n` / `Ctrl+x` | New pane / close pane |
| `Ctrl+q` | Quit Zellij |

All of these live in [config/config.kdl](config/config.kdl). See [docs/customizing.md](docs/customizing.md) for changing them.

## Project picker (Alt+s)

An fzf list of slots, registered projects, and recently modified folders under `projects_root`. Highlight a project and press:

| Key | Action |
|---|---|
| `1`..`8` | Put it in that slot. Live swap: the old panes close and new ones open without restarting. |
| `N` | Create a new project folder under `projects_root`, register it, assign a slot. |
| `S` | Pick a sound that plays when that project's Claude finishes a turn. macOS only, needs `.mp3` files in `~/.config/claude-sounds/`. |
| `F` | Pick a terminal font (Ghostty only). |
| `Esc` | Exit |

## Session lifecycle

```sh
zellij-workspace start [--grid 6|8]   # reap orphans, delete dead sessions, create or attach
zellij-workspace status               # session state, pane process counts, orphans
zellij-workspace nuke                 # kill all sessions and stray processes, then start fresh
```

`start` ignores SIGHUP, so closing the terminal window leaves the Zellij server running and `start` reattaches next time. Sessions are named `workspace-8` or `workspace-6`.

To launch straight into it, set your terminal's startup command to `zellij-workspace start` (Ghostty: the `command` line in [config/ghostty/config](config/ghostty/config)).

## Attention highlighting

In grid mode, a slot's frame turns blue when its Claude has finished a turn and is waiting on you. This is driven by two Claude Code hooks that touch and remove marker files the plugin polls. Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "Stop": [
      { "hooks": [ { "type": "command", "command": "workspace-claude-hook stop" } ] }
    ],
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": "workspace-claude-hook prompt" } ] }
    ]
  }
}
```

The hook finds the slot by matching Claude's working directory against `projects.json`. It exits silently outside the workspace, so it's safe to keep enabled globally.

## Repo layout

| Path | Purpose |
|---|---|
| `src/main.rs` | The Zellij plugin. Single file. |
| `scripts/zellij-workspace` | start / status / nuke |
| `scripts/zellij-swap` | Alt+s picker |
| `scripts/generate-layout.sh` | projects.json → `~/.config/zellij/layouts/workspace-{6,8}.kdl` |
| `scripts/claude-workspace` | What the Claude pane runs. Resumes the last conversation, honours `.envrc`. |
| `scripts/workspace-claude-hook` | Claude Code hook for attention highlighting |
| `scripts/zellij-urlpick` | Alt+u |
| `config/config.kdl` | Zellij keybindings and settings |
| `config/ghostty/config` | Ghostty settings that matter here |
| `config/projects.json.example` | Starting point for your projects.json |
| `docs/customizing.md` | Changing keys, panes, layout, terminal |
| `docs/architecture.md` | How the plugin and scripts work |
| `docs/history/` | Original design and implementation notes, per feature |
| `CLAUDE.md` | Orientation for Claude Code when working in this repo |

## Rebuilding the plugin

```sh
cargo build --release --target wasm32-wasip1
cp target/wasm32-wasip1/release/workspace-switcher.wasm ~/.config/zellij/plugins/workspace_switcher.wasm
zellij-workspace nuke && zellij-workspace start
```

Zellij caches plugins per session, so a restart is needed after rebuilding. There are no unit tests; the plugin is exercised live. Logs go to Zellij's stderr prefixed `[ws]`, visible with `zellij --debug` or in `/tmp/zellij-<uid>/zellij-log/`.

## Status

Extracted from a personal setup that ran daily for most of 2026. The generalisation (configurable pane commands, no hardcoded paths, the hook script) was done in one pass and compiles, but has not yet been exercised end to end on a fresh machine. Expect to touch things. [docs/architecture.md](docs/architecture.md) explains enough to debug it.
