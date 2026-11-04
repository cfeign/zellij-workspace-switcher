# Stop Hook Sound Picker Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a stop-hook sound picker to `zellij-swap` so users can set per-project notification sounds that play when Claude finishes (Stop hook). Integrated as an extra fzf key (`s`) in the existing project picker, with auto-preview of sounds.

**Tech Stack:** Bash (zellij-swap), fzf, jq, afplay (macOS), Claude Code `.claude/settings.json` hooks

---

## Step 1: Copy sounds to stable location

Copy the 50 Tonepaper audio files from `~/Downloads/` to `~/.config/claude-sounds/`.

```bash
mkdir -p ~/.config/claude-sounds
# Copy all .mp3 and .m4r files, flattening the directory structure
# Strip the prefix characters (- and _) and -Tonepaper suffix for clean display names
```

**Files to copy from:** `~/Downloads/Symplesound_Tonepaper-Rings/`
- `Tonpaper Rings - MP3 (Android)/Android Notifications MP3 - 2020/` (20 .mp3 files)
- `Tonpaper Rings - MP3 (Android)/Android Ringtones MP3 - 2020/` (30 .mp3 files)
- `Tonpaper Rings - M4R (iPhone)/iPhone Notifications M4R - 2020/` (20 .m4r files)
- `Tonpaper Rings - M4R (iPhone)/iPhone Ringtones M4R - 2020/` (30 .m4r files)

**Strategy:** Copy **only the MP3 notification sounds** (20 files — short sounds appropriate for stop hooks). Rename them to clean names:
- `- Cutie-Tonepaper.mp3` → `cutie.mp3`
- `- Ahah-Tonepaper.mp3` → `ahah.mp3`
- etc.

Ringtones are too long for notification sounds. M4R is iPhone-specific; MP3 works with `afplay`.

**Acceptance criteria:**
- `~/.config/claude-sounds/` contains 20 cleanly-named .mp3 files
- `afplay ~/.config/claude-sounds/cutie.mp3` plays successfully

---

## Step 2: Add sound indicator to zellij-swap display

In `build_list()`, for each slotted project, check if its `.claude/settings.json` contains a Stop hook with an `afplay` command. Show a `♪` indicator.

**Current display:**
```
[1] macbook-manager        main
[2] playatime              dev
```

**New display:**
```
[1] macbook-manager        main     ♪ cutie
[2] playatime              dev
[3] matrix-bot             main     ♪ ahah
```

**Implementation:**
```bash
# In build_list(), after getting branch info:
sound=""
settings_file="$path/.claude/settings.json"
if [[ -f "$settings_file" ]]; then
    afplay_cmd=$(jq -r '.hooks.Stop[]?.hooks[]? | select(.type == "command") | .command // empty' "$settings_file" 2>/dev/null | grep -m1 'afplay')
    if [[ -n "$afplay_cmd" ]]; then
        sound_file=$(echo "$afplay_cmd" | sed "s/.*afplay '\\(.*\\)'/\\1/" | xargs basename | sed 's/\..*//')
        sound="♪ $sound_file"
    fi
fi
printf "[%d] %-25s %-12s %s\n" "$i" "$proj" "$branch" "$sound"
```

**Acceptance criteria:**
- Projects with stop-hook sounds show `♪ soundname` in the picker
- Projects without sounds show nothing extra

---

## Step 3: Add `s` key to fzf for sound picker

Update the fzf `--expect` to include `s` alongside `1-8`:

```bash
selected=$(build_list | fzf --header="Select project → 1-8: assign slot | s: set sound | ESC: exit" \
    --expect=1,2,3,4,5,6,7,8,s \
    ...)
```

When `key == "s"`:
1. Resolve the highlighted project's path
2. Call `pick_sound "$proj_name" "$proj_path"` (new function)
3. Continue the loop (don't spawn panes)

**Acceptance criteria:**
- Pressing `s` on a project opens the sound sub-picker
- Pressing `1-8` still works as before (slot assignment)

---

## Step 4: Implement `pick_sound()` with auto-preview

New function in `zellij-swap`:

```bash
pick_sound() {
    local proj_name="$1"
    local proj_path="$2"
    local sounds_dir="$HOME/.config/claude-sounds"
    local preview_pid=""

    # Build sound list with "current" marker
    local current_sound=""
    local settings_file="$proj_path/.claude/settings.json"
    if [[ -f "$settings_file" ]]; then
        current_sound=$(jq -r '.hooks.Stop[]?.hooks[]? | select(.type == "command") | .command // empty' "$settings_file" 2>/dev/null \
            | grep -m1 'afplay' | sed "s/.*afplay '\\(.*\\)'/\\1/" | xargs -I{} basename {} | sed 's/\..*//')
    fi

    local list=""
    for f in "$sounds_dir"/*.mp3; do
        local name=$(basename "$f" .mp3)
        if [[ "$name" == "$current_sound" ]]; then
            list+="$name  (current)\n"
        else
            list+="$name\n"
        fi
    done
    list+="[none] - remove sound"

    # fzf with preview that plays audio
    local choice
    choice=$(echo -e "$list" | fzf \
        --header="Stop-hook sound for: $proj_name" \
        --preview="pkill -f 'afplay.*claude-sounds' 2>/dev/null; afplay '$sounds_dir/{1}.mp3' &" \
        --preview-window=hidden \
        --border=rounded \
        --margin=1 \
        --padding=1) || { pkill -f 'afplay.*claude-sounds' 2>/dev/null; return; }

    # Kill any lingering preview
    pkill -f 'afplay.*claude-sounds' 2>/dev/null

    if [[ -z "$choice" ]]; then
        return
    fi

    local sound_name=$(echo "$choice" | awk '{print $1}')

    if [[ "$sound_name" == "[none]" ]]; then
        remove_stop_sound "$proj_path"
        echo "Removed stop-hook sound for $proj_name"
    else
        set_stop_sound "$proj_path" "$sound_name"
        echo "Set stop-hook sound for $proj_name → $sound_name"
    fi
    sleep 0.5
}
```

**Auto-preview approach:** Use fzf's `--preview` to run `afplay` in background. The `--preview-window=hidden` hides the preview text output but still executes the command. Kill previous `afplay` before starting new one.

**Acceptance criteria:**
- Sound auto-plays when cursor moves to it
- Previous sound stops when cursor moves
- Pressing Enter selects the sound
- ESC cancels without changing anything
- `[none]` option removes existing sound

---

## Step 5: Implement `set_stop_sound()` and `remove_stop_sound()`

These functions write/modify the project's `.claude/settings.json`:

```bash
set_stop_sound() {
    local proj_path="$1"
    local sound_name="$2"
    local settings_file="$proj_path/.claude/settings.json"
    local sound_path="$HOME/.config/claude-sounds/${sound_name}.mp3"

    mkdir -p "$proj_path/.claude"

    if [[ ! -f "$settings_file" ]]; then
        # Create minimal settings with just the hook
        cat > "$settings_file" << ENDJSON
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "afplay '$sound_path'"
          }
        ]
      }
    ]
  }
}
ENDJSON
        return
    fi

    # File exists — use jq to upsert the Stop hook
    # Strategy: remove any existing afplay Stop hooks, then add the new one
    local tmp=$(mktemp)
    jq --arg cmd "afplay '$sound_path'" '
        # Ensure hooks object exists
        .hooks //= {} |
        # Remove existing Stop entries that contain afplay
        .hooks.Stop = [(.hooks.Stop // [])[] | select(.hooks | any(.command | test("afplay")) | not)] |
        # Add new Stop hook
        .hooks.Stop += [{"matcher": "", "hooks": [{"type": "command", "command": $cmd}]}]
    ' "$settings_file" > "$tmp" && mv "$tmp" "$settings_file"
}

remove_stop_sound() {
    local proj_path="$1"
    local settings_file="$proj_path/.claude/settings.json"

    if [[ ! -f "$settings_file" ]]; then
        return
    fi

    local tmp=$(mktemp)
    jq '
        .hooks.Stop = [(.hooks.Stop // [])[] | select(.hooks | any(.command | test("afplay")) | not)] |
        if (.hooks.Stop | length) == 0 then del(.hooks.Stop) else . end |
        if (.hooks | length) == 0 then del(.hooks) else . end
    ' "$settings_file" > "$tmp" && mv "$tmp" "$settings_file"
}
```

**Key behavior:**
- Only touches `afplay`-based Stop hooks — preserves any other Stop hooks (e.g., Python scripts)
- Creates `.claude/settings.json` if it doesn't exist
- Removes empty `hooks` and `Stop` keys on cleanup (no cruft)

**Acceptance criteria:**
- Setting a sound creates/updates the Stop hook with the correct `afplay` command
- Removing a sound only removes the afplay hook, preserving other hooks
- Works on projects that don't yet have `.claude/settings.json`

---

## Step 6: Fix stale `bv` references in zellij-swap

While we're editing `zellij-swap`, fix lines 160-162 which still reference `bv`:

```bash
# Line 160-162 (current):
zellij run -f --name "bv-${key}" --cwd "$proj_path" \
    -x "75%" -y "0%" --width "25%" --height "50%" \
    -- /opt/homebrew/bin/bv &

# Should be:
zellij run -f --name "iv-${key}" --cwd "$proj_path" \
    -x "75%" -y "0%" --width "25%" --height "50%" \
    -- ~/.local/bin/iv &
```

---

## File changes summary

| File | Action |
|------|--------|
| `~/.config/claude-sounds/*.mp3` | **Create** — 20 notification sounds copied from Downloads |
| `~/.local/bin/zellij-swap` | **Edit** — Add sound indicator, `s` key, `pick_sound()`, `set_stop_sound()`, `remove_stop_sound()`, fix bv→iv |

## Build order

1. Copy sounds (Step 1) — no dependencies
2. Edit `zellij-swap` (Steps 2-6) — single file, all changes together
3. Test: `Alt+S` → see sound indicators → press `s` → hear previews → select → verify `.claude/settings.json` updated
