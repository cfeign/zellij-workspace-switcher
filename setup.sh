#!/usr/bin/env bash
# One-shot setup: checks dependencies, builds the plugin, links scripts and
# config into place, creates projects.json from the example, generates layouts.
# Safe to re-run. Existing non-symlink files are backed up, never deleted.
#
#   git clone https://github.com/cfeign/zellij-workspace-switcher ~/code/zellij-workspace-switcher
#   cd ~/code/zellij-workspace-switcher && ./setup.sh
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

say()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mWARNING:\033[0m %s\n' "$*"; }
link() {
    local src="$1" dst="$2"
    mkdir -p "$(dirname "$dst")"
    if [[ -L "$dst" ]]; then rm "$dst"
    elif [[ -e "$dst" ]]; then say "backing up $dst -> $dst.bak"; mv "$dst" "$dst.bak"; fi
    ln -s "$src" "$dst"
    printf '    %s -> %s\n' "$dst" "$src"
}

# --- Dependencies -----------------------------------------------------------
say "checking dependencies"
missing=()
for dep in zellij jq fzf; do command -v "$dep" >/dev/null || missing+=("$dep"); done
if (( ${#missing[@]} )); then
    echo "Missing: ${missing[*]}"
    echo "  brew install ${missing[*]}"
    exit 1
fi
command -v claude >/dev/null || warn "claude (Claude Code) not on PATH; the claude panes will drop to a shell"
command -v fish   >/dev/null || warn "fish not found; set \"shell\" in projects.json to zsh or bash"

# --- Scripts ----------------------------------------------------------------
say "linking scripts into ~/.local/bin"
for script in zellij-workspace zellij-swap claude-workspace workspace-claude-hook zellij-urlpick; do
    link "$REPO_DIR/scripts/$script" "$HOME/.local/bin/$script"
done

# --- Zellij config ----------------------------------------------------------
say "linking zellij config"
link "$REPO_DIR/config/config.kdl" "$HOME/.config/zellij/config.kdl"

projects="$HOME/.config/zellij/projects.json"
if [[ ! -f "$projects" ]]; then
    cp "$REPO_DIR/config/projects.json.example" "$projects"
    say "created $projects from the example. Edit it, then re-run setup.sh (or generate-layout.sh)."
else
    say "keeping existing $projects"
fi

# --- Plugin -----------------------------------------------------------------
if command -v cargo >/dev/null; then
    if ! rustup target list --installed 2>/dev/null | grep -q wasm32-wasip1; then
        say "adding wasm32-wasip1 target"
        rustup target add wasm32-wasip1
    fi
    say "building plugin"
    (cd "$REPO_DIR" && cargo build --release --target wasm32-wasip1 2>&1 | tail -1)
    mkdir -p "$HOME/.config/zellij/plugins"
    cp "$REPO_DIR/target/wasm32-wasip1/release/workspace-switcher.wasm" \
       "$HOME/.config/zellij/plugins/workspace_switcher.wasm"
    echo "    installed ~/.config/zellij/plugins/workspace_switcher.wasm"
else
    warn "cargo not found; skipping plugin build. Install Rust (https://rustup.rs), then re-run."
fi

# --- Layouts ----------------------------------------------------------------
say "generating layouts"
"$REPO_DIR/scripts/generate-layout.sh" 8
"$REPO_DIR/scripts/generate-layout.sh" 6

# --- Optional extras --------------------------------------------------------
if command -v uv >/dev/null && ! command -v urlscan >/dev/null; then
    say "installing urlscan (for Alt+u fallback)"
    uv tool install urlscan 2>&1 | tail -1 || true
fi

cat <<EOF

Done. Next:
  1. Edit ~/.config/zellij/projects.json (projects, slots, grid 6|8, pane commands),
     then: $REPO_DIR/scripts/generate-layout.sh
  2. macOS + Ghostty: add 'macos-option-as-alt = true' to ~/.config/ghostty/config,
     or the Alt keybindings won't work. See config/ghostty/config.
  3. Start:  zellij-workspace start
  4. Optional attention highlighting: see README "Attention highlighting".
EOF
