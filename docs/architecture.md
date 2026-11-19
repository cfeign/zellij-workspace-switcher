# Architecture

Two halves: a Zellij WASM plugin that owns pane visibility and position, and bash scripts that own the session, the layout file, and the picker UI. They talk through pane titles, `zellij pipe`, and the layout's plugin configuration block.

## Pane naming convention

This is the contract everything hangs on. Every workspace pane has a title of the form

```
claude-<slot>-<project>      e.g. claude-3-my-app
git-<slot>                   e.g. git-3
issues-<slot>
test-<slot>
adhoc-<slot>
```

The layout generator writes these names. The plugin parses them from every `PaneUpdate` event to know which pane belongs to which slot. Nothing else is stored; if the titles are right, the plugin can be restarted or reset (`rescan`) at any time and rebuild its model.

## Plugin (`src/main.rs`)

A `zellij-tile` plugin with no UI. It subscribes to `PaneUpdate`, `Timer`, and `RunCommandResult`.

**State**

- `all_panes`: title → `PaneId`, rebuilt from scratch on every `PaneUpdate`
- `slots`: slot number → `SlotPanes { claude, git, issues, test, adhoc }`, rebuilt from `all_panes`
- `current_mode`: `Grid` or `Focus(slot)`
- `commands`: the command path for each pane type, read once from the layout's plugin configuration in `load()`
- `pending_renames`: fallback list of panes we opened during a swap and could not title by id
- `highlighted_slots`: which grid cells are currently highlighted

**Modes**

`apply_grid_mode` repositions every Claude pane to its grid cell (2x3 if the highest slot seen is 6 or less, else 2x4), shows them all, hides every tool pane.

`apply_focus_mode(slot)` moves that slot's Claude pane to the left half and its four tools to a 2x2 on the right, hides every other slot's panes, shows these five, focuses Claude.

Both use `change_floating_panes_coordinates`, `show_pane_with_id`, and `hide_pane_with_id`. Hiding is real visibility, not z-order, which is what makes 40 floating panes workable.

**Commands** arrive through `pipe()` from `zellij pipe "..."` or a `MessagePlugin` keybinding:

| Command | Effect |
|---|---|
| `grid` (alias `claude-only`) | grid mode |
| `focus N` | focus mode for slot N |
| `focus-pane <type>` | focus one pane in the current focus slot |
| `quick-focus N` | in grid: focus slot N's Claude pane; in focus: N selects a pane type (1 claude, 2 git, 3 issues, 4 test, 5 adhoc) |
| `swap-slot N name path` | close slot N's panes, open five new ones for the project at `path` |
| `close-slot N` | close slot N's panes |
| `rescan` | forget state and re-apply grid mode |

**Live swap and pane naming.** `swap-slot` calls `open_command_pane_floating` five times with the configured commands and cwd. Zellij titles a new command pane after its command, not after anything we ask for. The call returns the id of the new pane, so the plugin calls `rename_terminal_pane` on that id at once and stamps the convention name on. This is exact, also when several pane types run the same command (for example, three shells). The older `PendingRename` queue stays as a fallback for when Zellij returns no id: on the next `PaneUpdate`, each pending rename is matched to an unrecognised pane whose title contains the basename of the command launched for that pane type.

**Attention highlighting.** A 2-second timer runs `ls /tmp/claude-active/`. Files named `slot-N` mark slots whose Claude has finished a turn. The plugin diffs against `highlighted_slots` and calls `highlight_and_unhighlight_panes`. The files are written by `scripts/workspace-claude-hook` from Claude Code's `Stop` and `UserPromptSubmit` hooks, which resolve the slot from the working directory via projects.json.

**Configuration.** The layout passes commands as plugin config:

```kdl
plugin location="file:~/.config/zellij/plugins/workspace_switcher.wasm" {
    claude_cmd "/Users/me/.local/bin/claude-workspace"
    git_cmd "/opt/homebrew/bin/lazygit"
    issues_cmd "/opt/homebrew/bin/fish"
    test_cmd "/opt/homebrew/bin/fish"
    adhoc_cmd "/opt/homebrew/bin/fish"
}
```

`load()` reads them into `PaneCommands`. If a key is missing the plugin falls back to a bare name and hopes the Zellij server's PATH has it.

**Permissions** requested at load: `ReadApplicationState`, `ChangeApplicationState`, `OpenTerminalsOrPlugins`, `RunCommands`. Zellij prompts once per plugin path and remembers.

## Layout (`scripts/generate-layout.sh`)

Reads projects.json, resolves every pane command to an absolute path, and writes `~/.config/zellij/layouts/workspace-<grid>.kdl`: one tab, `hide_floating_panes=true`, a 1-row borderless plugin pane carrying the config block above, one tiled placeholder pane (a tab needs one), and five floating panes per slot named by convention with `cwd` set for occupied slots. Initial coordinates match grid mode for Claude panes and focus mode for tools; the plugin re-applies them on first `PaneUpdate` once it sees two or more slots.

## Session (`scripts/zellij-workspace`)

`start`: kill orphaned pane processes (PPID 1, name matching the configured pane commands), delete `EXITED` sessions, `trap '' HUP`, then `zellij attach workspace-N` if it's alive or `zellij --new-session-with-layout` if not. The HUP trap plus `on_force_close "detach"` in config.kdl is what keeps the server alive when the terminal window closes.

`nuke`: `kill-all-sessions`, wait, SIGKILL anything left.

`status`: session state and process counts.

## Picker (`scripts/zellij-swap`)

Runs in a floating pane from Alt+s. Builds an fzf list of slots (with current project and git branch), registered-but-unslotted projects, and recent folders under `projects_root`. `--expect` captures which key confirmed the selection. On a slot number it updates `slots` in projects.json, pipes `swap-slot` to the plugin, regenerates the layout so the next cold start matches, waits two seconds for panes to spawn, and pipes `rescan`.

## Why floating panes

Tiled panes can't overlap or be hidden individually, and resizing 40 of them is slow. Floating panes have explicit coordinates and per-pane visibility, so a mode switch is a batch of coordinate changes plus show/hide, which Zellij applies in one render. The cost is that the layout needs a dummy tiled pane and `hide_floating_panes=true` at startup so you don't see 40 panes stack before the plugin loads.
