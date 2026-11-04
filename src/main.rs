use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use zellij_tile::prelude::*;
use zellij_utils::input::layout::SplitSize;

/// Marker directory polled for active-Claude highlighting (see scripts/workspace-claude-hook)
const MARKER_DIR: &str = "/tmp/claude-active";

/// Panes belonging to a single workspace slot
#[derive(Debug, Default, Clone)]
struct SlotPanes {
    claude: Option<PaneId>,
    git: Option<PaneId>,
    issues: Option<PaneId>,
    test: Option<PaneId>,
    adhoc: Option<PaneId>,
}

impl SlotPanes {
    fn tool_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        if let Some(id) = self.git { ids.push(id); }
        if let Some(id) = self.issues { ids.push(id); }
        if let Some(id) = self.test { ids.push(id); }
        if let Some(id) = self.adhoc { ids.push(id); }
        ids
    }

    fn all_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        if let Some(id) = self.claude { ids.push(id); }
        ids.extend(self.tool_ids());
        ids
    }
}

/// Commands to launch in each pane type. Filled from the plugin's layout
/// configuration (written by scripts/generate-layout.sh from projects.json),
/// so nothing machine-specific lives in the binary.
#[derive(Debug, Clone)]
struct PaneCommands {
    claude: String,
    git: String,
    issues: String,
    test: String,
    adhoc: String,
}

impl Default for PaneCommands {
    fn default() -> Self {
        // Bare names: resolved via PATH of the zellij server if the layout
        // didn't provide absolute paths.
        PaneCommands {
            claude: "claude-workspace".to_string(),
            git: "sh".to_string(),
            issues: "sh".to_string(),
            test: "sh".to_string(),
            adhoc: "sh".to_string(),
        }
    }
}

impl PaneCommands {
    fn for_type(&self, t: &PaneType) -> &str {
        match t {
            PaneType::Claude => &self.claude,
            PaneType::Git => &self.git,
            PaneType::Issues => &self.issues,
            PaneType::Test => &self.test,
            PaneType::Adhoc => &self.adhoc,
        }
    }
}

/// Last path component of a command, used to recognise freshly opened panes
/// by the default title Zellij gives them (the command name).
fn basename(cmd: &str) -> &str {
    cmd.rsplit('/').next().unwrap_or(cmd)
}

/// Position and size for a floating pane (as percentages)
#[derive(Debug, Clone)]
struct PaneCoords {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Target display mode
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Grid,       // claude panes in grid (2x3 or 2x4), tools hidden
    Focus(u8),  // 1 claude pane + 4 floating tools
}

#[derive(Debug, Clone)]
struct PendingRename {
    desired_name: String,
    slot: u8,
    pane_type: PaneType,
}

#[derive(Debug, Clone, PartialEq)]
enum PaneType {
    Claude,
    Git,
    Issues,
    Test,
    Adhoc,
}

impl PaneType {
    /// Title prefix used in the pane naming convention (`git-3`, `claude-3-myproj`)
    fn prefix(&self) -> &'static str {
        match self {
            PaneType::Claude => "claude-",
            PaneType::Git => "git-",
            PaneType::Issues => "issues-",
            PaneType::Test => "test-",
            PaneType::Adhoc => "adhoc-",
        }
    }

    const ALL: [PaneType; 5] = [
        PaneType::Claude,
        PaneType::Git,
        PaneType::Issues,
        PaneType::Test,
        PaneType::Adhoc,
    ];
}

#[derive(Default)]
struct State {
    /// Commands for each pane type (from layout configuration)
    commands: PaneCommands,
    /// All known panes by title
    all_panes: HashMap<String, PaneId>,
    /// Slots map (rebuilt from all_panes)
    slots: HashMap<u8, SlotPanes>,
    /// Current mode
    current_mode: Option<Mode>,
    /// Whether grid mode has been auto-applied on startup
    initialized: bool,
    /// Pending pane renames after a swap - maps expected title to (command_path, cwd)
    pending_renames: Vec<PendingRename>,
    /// Slots currently highlighted (active Claude panes)
    highlighted_slots: HashSet<u8>,
    /// Whether the highlight polling timer has been started
    timer_started: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        eprintln!("[ws] Plugin loading - v4 (configurable commands)");
        // Pane commands come from the layout's plugin block, e.g.
        //   plugin location="file:..." { claude_cmd "/path/claude-workspace"; git_cmd "/path/lazygit" }
        let get = |key: &str, fallback: &str| -> String {
            configuration
                .get(key)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| fallback.to_string())
        };
        let defaults = PaneCommands::default();
        self.commands = PaneCommands {
            claude: get("claude_cmd", &defaults.claude),
            git: get("git_cmd", &defaults.git),
            issues: get("issues_cmd", &defaults.issues),
            test: get("test_cmd", &defaults.test),
            adhoc: get("adhoc_cmd", &defaults.adhoc),
        };
        eprintln!("[ws] Commands: {:?}", self.commands);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::RunCommands,
        ]);
        subscribe(&[
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::Timer,
            EventType::RunCommandResult,
        ]);
        set_timeout(2.0);
        self.timer_started = true;
        // Clean slate: remove any stale highlight marker files from previous sessions
        run_command(
            &["sh", "-c", &format!("mkdir -p {d} && rm -f {d}/slot-*", d = MARKER_DIR)],
            BTreeMap::new(),
        );
        eprintln!("[ws] Plugin loaded");
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => {
                self.update_pane_map(&manifest);
                true
            }
            Event::Timer(_) => {
                // List all slot files (needs review markers persist until next UserPromptSubmit)
                run_command(
                    &["ls", MARKER_DIR],
                    BTreeMap::from([("source".to_string(), "highlight-poll".to_string())]),
                );
                set_timeout(2.0);
                false
            }
            Event::RunCommandResult(exit_code, stdout, _stderr, context) => {
                if context.get("source").map(|s| s.as_str()) == Some("highlight-poll") {
                    self.process_highlight_poll(exit_code, &stdout);
                }
                false
            }
            Event::PermissionRequestResult(_) => false,
            _ => false,
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if let Some(payload) = pipe_message.payload {
            eprintln!("[ws] Command: {}", payload);
            self.handle_command(&payload);
            true
        } else {
            false
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl State {
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

        // Process pending renames: find unrecognized panes and rename them
        if !self.pending_renames.is_empty() {
            // Find unrecognized terminal panes
            let mut unrecognized: Vec<(String, u32)> = Vec::new();
            for (title, &pane_id) in &self.all_panes {
                let is_known = PaneType::ALL.iter().any(|t| title.starts_with(t.prefix()));
                if !is_known {
                    if let PaneId::Terminal(id) = pane_id {
                        unrecognized.push((title.clone(), id));
                    }
                }
            }

            if !unrecognized.is_empty() {
                eprintln!("[ws] Found {} unrecognized panes, {} pending renames",
                    unrecognized.len(), self.pending_renames.len());

                // A freshly opened pane is titled after its command, so match
                // each pending rename against the basename of the command we
                // launched for that pane type.
                let commands = self.commands.clone();
                let mut remaining_renames = Vec::new();
                for rename in self.pending_renames.drain(..) {
                    let needle = basename(commands.for_type(&rename.pane_type)).to_string();
                    let match_idx = unrecognized.iter().position(|(title, _)| {
                        title.contains(&needle)
                            || (rename.pane_type == PaneType::Claude && title.contains("claude"))
                    });

                    if let Some(idx) = match_idx {
                        let (old_title, terminal_id) = unrecognized.remove(idx);
                        eprintln!("[ws] Renaming pane {} ({}) to {}", terminal_id, old_title, rename.desired_name);
                        rename_terminal_pane(terminal_id, &rename.desired_name);
                    } else {
                        remaining_renames.push(rename);
                    }
                }
                self.pending_renames = remaining_renames;

                if !self.pending_renames.is_empty() {
                    eprintln!("[ws] {} pending renames still unmatched", self.pending_renames.len());
                }
            }
        }

        // Auto-apply grid mode once we detect enough slots
        if !self.initialized && self.slots.len() >= 2 {
            eprintln!("[ws] Auto-initializing grid mode with {} slots", self.slots.len());
            self.apply_grid_mode();
            self.initialized = true;
        }
    }

    fn rebuild_slots(&mut self) {
        self.slots.clear();

        for (title, &pane_id) in &self.all_panes {
            for t in PaneType::ALL.iter() {
                if !title.starts_with(t.prefix()) {
                    continue;
                }
                if let Some(slot) = self.parse_slot(title, t.prefix()) {
                    let entry = self.slots.entry(slot).or_default();
                    match t {
                        PaneType::Claude => entry.claude = Some(pane_id),
                        PaneType::Git => entry.git = Some(pane_id),
                        PaneType::Issues => entry.issues = Some(pane_id),
                        PaneType::Test => entry.test = Some(pane_id),
                        PaneType::Adhoc => entry.adhoc = Some(pane_id),
                    }
                }
                break;
            }
        }

        eprintln!("[ws] Slots: {:?}", self.slots.keys().collect::<Vec<_>>());
    }

    fn parse_slot(&self, title: &str, prefix: &str) -> Option<u8> {
        let after_prefix = &title[prefix.len()..];
        let num_str = after_prefix.split('-').next()?;
        num_str.parse().ok()
    }

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
                    if (1..=self.max_slot()).contains(&slot) {
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
                // Must be in focus mode to target a pane
                if let Some(Mode::Focus(slot)) = self.current_mode {
                    if let Some(slot_panes) = self.slots.get(&slot) {
                        let pane_id = match *pane_type {
                            "claude" => slot_panes.claude,
                            "git" => slot_panes.git,
                            "issues" => slot_panes.issues,
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
                    eprintln!("[ws] Not in focus mode - use Alt+N first");
                }
            }
            ["quick-focus", n_str] => {
                if let Ok(n) = n_str.parse::<u8>() {
                    match &self.current_mode {
                        Some(Mode::Grid) => {
                            // Focus grid cell N without leaving grid
                            if let Some(slot_panes) = self.slots.get(&n) {
                                if let Some(id) = slot_panes.claude {
                                    focus_pane_with_id(id, false);
                                    eprintln!("[ws] Quick-focus slot {} (grid)", n);
                                }
                            }
                        }
                        Some(Mode::Focus(slot)) => {
                            // Map number to pane type within current focused slot
                            let pane_id = if let Some(slot_panes) = self.slots.get(slot) {
                                match n {
                                    1 => slot_panes.claude,
                                    2 => slot_panes.git,
                                    3 => slot_panes.issues,
                                    4 => slot_panes.test,
                                    5 => slot_panes.adhoc,
                                    _ => None,
                                }
                            } else {
                                None
                            };
                            if let Some(id) = pane_id {
                                focus_pane_with_id(id, false);
                                eprintln!("[ws] Quick-focus pane {} in slot {} (focus)", n, slot);
                            }
                        }
                        None => {}
                    }
                }
            }
            ["close-slot", slot_str] => {
                if let Ok(slot) = slot_str.parse::<u8>() {
                    if (1..=self.max_slot()).contains(&slot) {
                        if let Some(slot_panes) = self.slots.get(&slot) {
                            let ids = slot_panes.all_ids();
                            if !ids.is_empty() {
                                eprintln!("[ws] Closing {} panes for slot {}", ids.len(), slot);
                                close_multiple_panes(ids);
                            }
                        }
                        self.slots.remove(&slot);
                        self.highlighted_slots.remove(&slot);
                        // Clean up the highlight marker file
                        run_command(
                            &["rm", "-f", &format!("{}/slot-{}", MARKER_DIR, slot)],
                            BTreeMap::new(),
                        );
                        eprintln!("[ws] Slot {} closed", slot);
                    }
                }
            }
            ["swap-slot", slot_str, project_name, project_path] => {
                if let Ok(slot) = slot_str.parse::<u8>() {
                    if (1..=self.max_slot()).contains(&slot) {
                        eprintln!("[ws] -> Swap slot {} to {} ({})", slot, project_name, project_path);
                        self.swap_slot(slot, project_name, project_path);
                    } else {
                        eprintln!("[ws] Invalid slot number: {}", slot);
                    }
                } else {
                    eprintln!("[ws] Invalid slot number: {}", slot_str);
                }
            }
            ["rescan"] => {
                eprintln!("[ws] Rescan requested - resetting state");
                self.initialized = false;
                self.current_mode = None;
                // Force a grid mode apply now with current data
                self.apply_grid_mode();
                self.initialized = true;
            }
            _ => eprintln!("[ws] Unknown command: {:?}", parts),
        }
    }

    /// Grid mode: Show all 8 claude panes in 2x4, hide all tools
    fn apply_grid_mode(&mut self) {
        // Build coordinate map for grid layout
        let grid_coords = self.build_grid_coordinates();

        // Reposition all claude panes
        let mut reposition_commands = Vec::new();
        for (&slot, coords) in &grid_coords {
            if let Some(pane_panes) = self.slots.get(&slot) {
                if let Some(claude_id) = pane_panes.claude {
                    reposition_commands.push((claude_id, self.coords_to_floating(&coords)));
                }
            }
        }

        if !reposition_commands.is_empty() {
            change_floating_panes_coordinates(reposition_commands);
        }

        // Show all claude panes
        for slot_panes in self.slots.values() {
            if let Some(id) = slot_panes.claude {
                show_pane_with_id(id, false);
            }
        }

        // Hide all tool panes (they're floating, so just hide them)
        for slot_panes in self.slots.values() {
            for id in slot_panes.tool_ids() {
                hide_pane_with_id(id);
            }
        }

        self.current_mode = Some(Mode::Grid);
        eprintln!("[ws] Grid mode applied");
    }

    /// Focus mode: Show 1 claude pane (left 50%) + 4 floating tools (right 50% in 2x2)
    fn apply_focus_mode(&mut self, slot: u8) {
        let target_panes = match self.slots.get(&slot) {
            Some(panes) => panes.clone(),
            None => {
                eprintln!("[ws] Slot {} not found", slot);
                return;
            }
        };

        let mut reposition_commands = Vec::new();

        // Reposition target claude pane to left 50% full height
        if let Some(claude_id) = target_panes.claude {
            reposition_commands.push((claude_id, self.coords_to_floating(&tool_coords(&PaneType::Claude))));
        }

        // Reposition tools in right 50% (2x2 grid)
        let tools = [
            (target_panes.git, PaneType::Git),
            (target_panes.issues, PaneType::Issues),
            (target_panes.test, PaneType::Test),
            (target_panes.adhoc, PaneType::Adhoc),
        ];
        for (id, t) in tools.iter() {
            if let Some(id) = id {
                reposition_commands.push((*id, self.coords_to_floating(&tool_coords(t))));
            }
        }

        if !reposition_commands.is_empty() {
            change_floating_panes_coordinates(reposition_commands);
        }

        // Hide all other claude panes
        for (&slot_num, slot_panes) in &self.slots {
            if slot_num != slot {
                if let Some(id) = slot_panes.claude {
                    hide_pane_with_id(id);
                }
            }
        }

        // Show target claude pane (but don't focus yet)
        if let Some(id) = target_panes.claude {
            show_pane_with_id(id, false);
        }

        // Hide all other slots' tools
        for (&slot_num, slot_panes) in &self.slots {
            if slot_num != slot {
                for id in slot_panes.tool_ids() {
                    hide_pane_with_id(id);
                }
            }
        }

        // Show target slot's tools
        for id in target_panes.tool_ids() {
            show_pane_with_id(id, false);
        }

        // Focus the claude pane after all visibility changes
        if let Some(id) = target_panes.claude {
            focus_pane_with_id(id, false);
        }

        self.current_mode = Some(Mode::Focus(slot));
        eprintln!("[ws] Focus mode slot {} applied", slot);
    }

    /// Max slot number detected from current pane layout
    fn max_slot(&self) -> u8 {
        self.slots.keys().max().copied().unwrap_or(8)
    }

    /// Build grid coordinates dynamically:
    /// ≤6 slots → 2x3 (33/34/33% widths)
    /// >6 slots → 2x4 (25% each)
    fn build_grid_coordinates(&self) -> HashMap<u8, PaneCoords> {
        let max_slot = self.max_slot();
        let mut coords = HashMap::new();

        if max_slot <= 6 {
            // 2x3 grid: 33/34/33% widths
            let xs = [0.0, 33.0, 67.0];
            let widths = [33.0, 34.0, 33.0];
            for slot in 1..=max_slot {
                let col = ((slot - 1) % 3) as usize;
                let row = ((slot - 1) / 3) as usize;
                coords.insert(slot, PaneCoords {
                    x: xs[col], y: row as f64 * 50.0,
                    width: widths[col], height: 50.0,
                });
            }
        } else {
            // 2x4 grid: 25% each
            for slot in 1..=max_slot {
                let col = ((slot - 1) % 4) as usize;
                let row = ((slot - 1) / 4) as usize;
                coords.insert(slot, PaneCoords {
                    x: col as f64 * 25.0, y: row as f64 * 50.0,
                    width: 25.0, height: 50.0,
                });
            }
        }

        coords
    }

    /// Convert PaneCoords to FloatingPaneCoordinates
    fn coords_to_floating(&self, coords: &PaneCoords) -> FloatingPaneCoordinates {
        coords_to_floating(coords)
    }
}

/// Focus-mode position of each tool pane: right half of the screen, 2x2.
fn tool_coords(t: &PaneType) -> PaneCoords {
    match t {
        PaneType::Git => PaneCoords { x: 50.0, y: 0.0, width: 25.0, height: 50.0 },
        PaneType::Issues => PaneCoords { x: 75.0, y: 0.0, width: 25.0, height: 50.0 },
        PaneType::Test => PaneCoords { x: 50.0, y: 50.0, width: 25.0, height: 50.0 },
        PaneType::Adhoc => PaneCoords { x: 75.0, y: 50.0, width: 25.0, height: 50.0 },
        PaneType::Claude => PaneCoords { x: 0.0, y: 0.0, width: 50.0, height: 100.0 },
    }
}

fn coords_to_floating(coords: &PaneCoords) -> FloatingPaneCoordinates {
    FloatingPaneCoordinates {
        x: Some(SplitSize::Percent(coords.x as usize)),
        y: Some(SplitSize::Percent(coords.y as usize)),
        width: Some(SplitSize::Percent(coords.width as usize)),
        height: Some(SplitSize::Percent(coords.height as usize)),
        pinned: None,
    }
}

impl State {
    fn process_highlight_poll(&mut self, exit_code: Option<i32>, stdout: &[u8]) {
        let mut active_slots: HashSet<u8> = HashSet::new();
        if exit_code == Some(0) {
            let output = String::from_utf8_lossy(stdout);
            for line in output.lines() {
                if let Some(num_str) = line.trim().strip_prefix("slot-") {
                    if let Ok(slot) = num_str.parse::<u8>() {
                        if (1..=self.max_slot()).contains(&slot) {
                            active_slots.insert(slot);
                        }
                    }
                }
            }
        }

        if active_slots == self.highlighted_slots {
            return;
        }

        let to_highlight: Vec<PaneId> = active_slots
            .iter()
            .filter(|s| !self.highlighted_slots.contains(s))
            .filter_map(|s| self.slots.get(s).and_then(|sp| sp.claude))
            .collect();
        let to_unhighlight: Vec<PaneId> = self
            .highlighted_slots
            .iter()
            .filter(|s| !active_slots.contains(s))
            .filter_map(|s| self.slots.get(s).and_then(|sp| sp.claude))
            .collect();

        if !to_highlight.is_empty() || !to_unhighlight.is_empty() {
            eprintln!(
                "[ws] Highlight update: +{:?} -{:?}",
                active_slots
                    .difference(&self.highlighted_slots)
                    .collect::<Vec<_>>(),
                self.highlighted_slots
                    .difference(&active_slots)
                    .collect::<Vec<_>>()
            );
            highlight_and_unhighlight_panes(to_highlight, to_unhighlight);
        }

        self.highlighted_slots = active_slots;
    }

    /// Swap a slot's panes to a new project: close old panes, open new ones
    fn swap_slot(&mut self, slot: u8, project_name: &str, project_path: &str) {
        let cwd = PathBuf::from(project_path);

        // Close existing panes for this slot
        if let Some(slot_panes) = self.slots.get(&slot) {
            let ids = slot_panes.all_ids();
            if !ids.is_empty() {
                eprintln!("[ws] Closing {} panes for slot {}", ids.len(), slot);
                close_multiple_panes(ids);
            }
        }
        // Clean up the old slot's highlight marker file
        run_command(
            &["rm", "-f", &format!("{}/slot-{}", MARKER_DIR, slot)],
            BTreeMap::new(),
        );

        // Grid coordinates for the claude pane
        let grid_coords = self.build_grid_coordinates();
        let claude_coords = grid_coords.get(&slot).cloned().unwrap_or(
            PaneCoords { x: 0.0, y: 0.0, width: 25.0, height: 50.0 }
        );

        // Open one pane per type, in the order the layout generator uses.
        // Each gets the command configured for its type and a queued rename
        // so the next PaneUpdate can stamp the naming convention onto it.
        for t in PaneType::ALL.iter() {
            let name = match t {
                PaneType::Claude => format!("claude-{}-{}", slot, project_name),
                _ => format!("{}{}", t.prefix(), slot),
            };
            let coords = match t {
                PaneType::Claude => claude_coords.clone(),
                _ => tool_coords(t),
            };
            let mut ctx = BTreeMap::new();
            ctx.insert("name".to_string(), name.clone());
            open_command_pane_floating(
                CommandToRun {
                    path: PathBuf::from(self.commands.for_type(t)),
                    args: vec![],
                    cwd: Some(cwd.clone()),
                },
                Some(coords_to_floating(&coords)),
                ctx,
            );
            self.pending_renames.push(PendingRename {
                desired_name: name,
                slot,
                pane_type: t.clone(),
            });
        }

        // Remove old slot data - will be rebuilt on next PaneUpdate
        self.slots.remove(&slot);
        self.highlighted_slots.remove(&slot);
        eprintln!("[ws] Slot {} swapped to {} ({})", slot, project_name, project_path);
    }
}
