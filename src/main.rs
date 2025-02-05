use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;
use std::time::Duration;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

/// Returns a centered rectangle with the given width and height percentages of the given rect.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);
    let vertical_chunk = popup_layout[1];
    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(vertical_chunk);
    horizontal_layout[1]
}

/// Returns the configuration directory based on the OS.
fn config_path() -> &'static str {
    match std::env::consts::OS {
        "windows" => r"C:\ProgramData\rustguard\wireguard\",
        "macos" => "/usr/local/etc/wireguard/",
        _ => "/etc/wireguard/",
    }
}

/// Returns the log file path based on the OS.
fn log_file_path() -> &'static str {
    match std::env::consts::OS {
        "windows" => r"C:\ProgramData\rustguard\rustguard.log",
        "macos" => "/usr/local/var/log/rustguard.log",
        _ => "/var/log/rustguard.log",
    }
}

/// Write a persistent log entry.
fn log_status(message: &str) {
    let log_path = log_file_path();
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(log_path)
        .expect("Failed to open log file");
    writeln!(file, "{}", message).expect("Failed to write to log file");
}

/// List all VPN profiles (config files) in the configuration directory (without the ".conf" suffix).
fn list_vpn_profiles() -> Vec<String> {
    let path = config_path();
    if let Ok(entries) = fs::read_dir(path) {
        entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().into_string().unwrap())
            .filter(|name| name.ends_with(".conf"))
            .map(|name| name.trim_end_matches(".conf").to_string())
            .collect()
    } else {
        vec![]
    }
}

/// Toggle the VPN connection using "wg-quick up/down".
/// For the "up" action, check that the configuration file is not empty.
fn toggle_vpn(profile: &str, action: &str) -> String {
    if action == "up" {
        let filename = format!("{}{}.conf", config_path(), profile);
        if let Ok(content) = fs::read_to_string(&filename) {
            if content.trim().is_empty() {
                return format!("❌ Failed to start VPN: configuration file {} is empty.", filename);
            }
        } else {
            return format!("❌ Failed to read configuration file {}.", filename);
        }
    }

    let output = Command::new("sudo")
        .arg("wg-quick")
        .arg(action)
        .arg(profile)
        .output()
        .expect("Failed to execute wg-quick command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        format!("✅ {} VPN {} successfully\n{}", profile, action, stdout)
    } else {
        format!("❌ Failed to {} VPN {}:\n{}", action, profile, stderr)
    }
}

/// Get active VPN interfaces by parsing "wg show" output.
fn get_active_vpns() -> Vec<String> {
    let output = Command::new("wg")
        .arg("show")
        .output()
        .expect("Failed to get VPN status");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut active_vpns = Vec::new();

    for line in stdout.lines() {
        if line.contains("interface:") {
            if let Some(interface) = line.split_whitespace().nth(1) {
                active_vpns.push(interface.to_string());
            }
        }
    }
    active_vpns
}

/// Get full details for a VPN interface (using "wg show <interface>").
fn get_vpn_details(interface: &str) -> String {
    let output = Command::new("wg")
        .arg("show")
        .arg(interface)
        .output()
        .expect("Failed to get VPN details");
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Minimal Vim–like editor mode.
#[derive(Clone, Debug, PartialEq)]
enum EditorMode {
    Normal,
    Insert,
}

/// A minimal multi–line editor state.
#[derive(Clone)]
struct EditorState {
    profile: String,
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    mode: EditorMode,
    /// When true, the editor overlay cheatsheet is visible.
    show_cheatsheet: bool,
}

impl EditorState {
    fn new(profile: String, content: String) -> Self {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|l| l.to_string()).collect()
        };
        Self {
            profile,
            lines,
            cursor_row: 0,
            cursor_col: 0,
            mode: EditorMode::Normal,
            show_cheatsheet: false,
        }
    }

    /// Handle key events while editing.
    ///
    /// Returns Some(result) if editing is finished:
    /// - Some("saved") if the user pressed Ctrl+S to save,
    /// - Some("cancel") if the user pressed Esc (in Normal mode) to cancel.
    /// Otherwise returns None.
    fn handle_event(&mut self, key: KeyEvent) -> Option<&'static str> {
        match self.mode {
            EditorMode::Normal => {
                if self.show_cheatsheet {
                    self.show_cheatsheet = false;
                    return None;
                }
                match key.code {
                    KeyCode::Char('h') | KeyCode::Left => {
                        if self.cursor_col > 0 {
                            self.cursor_col -= 1;
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        if let Some(line) = self.lines.get(self.cursor_row) {
                            if self.cursor_col < line.len() {
                                self.cursor_col += 1;
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.cursor_row > 0 {
                            self.cursor_row -= 1;
                            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if self.cursor_row + 1 < self.lines.len() {
                            self.cursor_row += 1;
                            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
                        }
                    }
                    KeyCode::Char('i') => {
                        self.mode = EditorMode::Insert;
                        execute!(std::io::stdout(), cursor::Show).ok();
                    }
                    KeyCode::Char('a') => {
                        if let Some(line) = self.lines.get(self.cursor_row) {
                            if self.cursor_col < line.len() {
                                self.cursor_col += 1;
                            }
                        }
                        self.mode = EditorMode::Insert;
                        execute!(std::io::stdout(), cursor::Show).ok();
                    }
                    KeyCode::Char('o') => {
                        self.cursor_row += 1;
                        self.lines.insert(self.cursor_row, String::new());
                        self.cursor_col = 0;
                        self.mode = EditorMode::Insert;
                        execute!(std::io::stdout(), cursor::Show).ok();
                    }
                    KeyCode::Char('x') => {
                        if let Some(line) = self.lines.get_mut(self.cursor_row) {
                            if self.cursor_col < line.len() {
                                line.remove(self.cursor_col);
                            }
                        }
                    }
                    KeyCode::Char('D') => {
                        if self.lines.len() > 1 {
                            self.lines.remove(self.cursor_row);
                            if self.cursor_row >= self.lines.len() {
                                self.cursor_row = self.lines.len() - 1;
                            }
                            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
                        } else {
                            self.lines[0].clear();
                            self.cursor_col = 0;
                        }
                    }
                    KeyCode::Char('?') => {
                        self.show_cheatsheet = !self.show_cheatsheet;
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Some("saved");
                    }
                    KeyCode::Esc => return Some("cancel"),
                    _ => {}
                }
            }
            EditorMode::Insert => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = EditorMode::Normal;
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Some(line) = self.lines.get_mut(self.cursor_row) {
                            line.insert(self.cursor_col, c);
                            self.cursor_col += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(line) = self.lines.get_mut(self.cursor_row) {
                            let new_line = line.split_off(self.cursor_col);
                            self.lines.insert(self.cursor_row + 1, new_line);
                            self.cursor_row += 1;
                            self.cursor_col = 0;
                        }
                    }
                    KeyCode::Backspace => {
                        if self.cursor_col > 0 {
                            if let Some(line) = self.lines.get_mut(self.cursor_row) {
                                line.remove(self.cursor_col - 1);
                                self.cursor_col -= 1;
                            }
                        } else if self.cursor_row > 0 {
                            let current_line = self.lines.remove(self.cursor_row);
                            self.cursor_row -= 1;
                            self.cursor_col = self.lines[self.cursor_row].len();
                            self.lines[self.cursor_row].push_str(&current_line);
                        }
                    }
                    KeyCode::Left => {
                        if self.cursor_col > 0 {
                            self.cursor_col -= 1;
                        } else if self.cursor_row > 0 {
                            self.cursor_row -= 1;
                            self.cursor_col = self.lines[self.cursor_row].len();
                        }
                    }
                    KeyCode::Right => {
                        if let Some(line) = self.lines.get(self.cursor_row) {
                            if self.cursor_col < line.len() {
                                self.cursor_col += 1;
                            } else if self.cursor_row + 1 < self.lines.len() {
                                self.cursor_row += 1;
                                self.cursor_col = 0;
                            }
                        }
                    }
                    KeyCode::Up => {
                        if self.cursor_row > 0 {
                            self.cursor_row -= 1;
                            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
                        }
                    }
                    KeyCode::Down => {
                        if self.cursor_row + 1 < self.lines.len() {
                            self.cursor_row += 1;
                            self.cursor_col = self.cursor_col.min(self.lines[self.cursor_row].len());
                        }
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Some("saved");
                    }
                    _ => {}
                }
            }
        }
        None
    }
}

/// All the screens our application can show.
enum Screen {
    Manager,    // Main manager UI
    Status,     // Persistent status log
    Help,       // Global keybindings help
    Details { interface: String, details: String }, // VPN details view
    Editor(EditorState), // Config editor
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let profiles = list_vpn_profiles();
    let mut selected_index: usize = 0;
    let mut status_log: Vec<String> = Vec::new();
    let mut screen = Screen::Manager;

    loop {
        let active_vpns = get_active_vpns();

        terminal.draw(|f| {
            let area = f.area();
            match &screen {
                Screen::Manager => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(
                            [
                                Constraint::Percentage(60),
                                Constraint::Percentage(20),
                                Constraint::Percentage(10),
                                Constraint::Percentage(10),
                            ]
                            .as_ref(),
                        )
                        .split(area);

                    let items: Vec<ListItem> = profiles
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let is_active = active_vpns.contains(p);
                            let style = if i == selected_index {
                                if is_active {
                                    Style::default()
                                        .fg(Color::Green)
                                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                                } else {
                                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
                                }
                            } else if is_active {
                                Style::default().fg(Color::Green)
                            } else {
                                Style::default()
                            };
                            ListItem::new(p.clone()).style(style)
                        })
                        .collect();

                    let block = Block::default()
                        .title(" WireGuard Manager (Press S: Status, H: Help) ")
                        .borders(Borders::ALL);
                    let list = List::new(items).block(block);
                    f.render_widget(list, chunks[0]);

                    let active_conns = Paragraph::new(
                        if active_vpns.is_empty() {
                            "None".into()
                        } else {
                            active_vpns.join(", ")
                        },
                    )
                    .block(Block::default().borders(Borders::ALL).title(" Active Connections "));
                    f.render_widget(active_conns, chunks[1]);

                    let instructions = Paragraph::new(
                        "↑/k, ↓/j: Navigate | Enter: Connect/Disconnect | D: Details | \
                         E: Edit Config | Q: Quit",
                    )
                    .block(Block::default().borders(Borders::ALL));
                    f.render_widget(instructions, chunks[2]);

                    let latest = status_log.last().cloned().unwrap_or_else(|| "No actions yet".into());
                    let status = Paragraph::new(latest)
                        .block(Block::default().borders(Borders::ALL).title(" Latest Status "));
                    f.render_widget(status, chunks[3]);
                }
                Screen::Status => {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
                        .split(area);

                    let log_items: Vec<ListItem> = status_log
                        .iter()
                        .map(|entry| ListItem::new(entry.clone()))
                        .collect();

                    let block = Block::default()
                        .title(" Status Log (Press W: Manager, Q: Quit) ")
                        .borders(Borders::ALL);
                    let list = List::new(log_items).block(block);
                    f.render_widget(list, chunks[0]);
                    let instructions = Paragraph::new("W: Return to Manager | Q: Quit")
                        .block(Block::default().borders(Borders::ALL));
                    f.render_widget(instructions, chunks[1]);
                }
                Screen::Help => {
                    let help_message = "RustGuard Global Keybindings:
↑/k, ↓/j: Navigate
Enter: Connect/Disconnect VPN
D: VPN Details
E: Edit Config
S: View Status Log
W: WireGuard Manager
H: Show Help
Q: Quit

Press any key to return.";
                    let block = Block::default().title(" Help ").borders(Borders::ALL);
                    let paragraph = Paragraph::new(help_message).block(block);
                    f.render_widget(paragraph, area);
                }
                Screen::Details { interface, details } => {
                    let block = Block::default()
                        .title(format!(" VPN Details: {} (Press any key to return)", interface))
                        .borders(Borders::ALL);
                    let paragraph = Paragraph::new(details.clone()).block(block);
                    f.render_widget(paragraph, area);
                }
                Screen::Editor(editor_state) => {
                    let content = editor_state.lines.join("\n");
                    let block = Block::default().title(format!(
                        " Editing {}{}.conf (Ctrl+S: Save, Esc: Cancel) ",
                        config_path(),
                        editor_state.profile
                    ))
                    .borders(Borders::ALL);
                    let paragraph = Paragraph::new(content).block(block);
                    f.render_widget(paragraph, area);

                    let mode_str = match editor_state.mode {
                        EditorMode::Normal => "NORMAL",
                        EditorMode::Insert => "INSERT",
                    };
                    let footer_text = format!(
                        "Mode: {} | Line: {} Col: {}",
                        mode_str,
                        editor_state.cursor_row + 1,
                        editor_state.cursor_col + 1
                    );
                    let footer_area = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
                        .split(area);
                    let footer = Paragraph::new(footer_text)
                        .style(Style::default().fg(Color::Yellow));
                    f.render_widget(footer, footer_area[1]);

                    if editor_state.show_cheatsheet {
                        let help_text = "Editor Cheatsheet (Normal mode):
i      : Enter Insert mode
a      : Append (move right then insert)
o      : Open new line below
h/j/k/l or ←/↓/↑/→: Move cursor
x      : Delete character under cursor
D      : Delete current line
?      : Toggle this help
Ctrl+S : Save and exit
Esc    : Cancel editing / return to Normal mode
Press any key (in Normal mode) to hide this help.";
                        let overlay_area = centered_rect(60, 40, area);
                        let help_block = Block::default().title("Editor Help").borders(Borders::ALL);
                        let help_paragraph = Paragraph::new(help_text)
                            .block(help_block)
                            .style(Style::default().fg(Color::Magenta));
                        f.render_widget(help_paragraph, overlay_area);
                    }

                    let cursor_x = area.x + editor_state.cursor_col as u16 + 1;
                    let cursor_y = area.y + editor_state.cursor_row as u16 + 1;
                    f.set_cursor_position((cursor_x, cursor_y));
                }
            }
        })?;

        if event::poll(Duration::from_millis(200))? {
            let ev = event::read()?;
            match &mut screen {
                Screen::Manager => {
                    if let Event::Key(key) = ev {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('s') => { screen = Screen::Status; }
                            KeyCode::Char('h') => { screen = Screen::Help; }
                            KeyCode::Char('d') => {
                                if profiles.is_empty() { continue; }
                                let selected = profiles[selected_index].clone();
                                let details = get_vpn_details(&selected);
                                screen = Screen::Details { interface: selected, details };
                            }
                            KeyCode::Char('e') => {
                                if profiles.is_empty() { continue; }
                                let selected = profiles[selected_index].clone();
                                let filename = format!("{}{}.conf", config_path(), selected);
                                let content = fs::read_to_string(&filename).unwrap_or_default();
                                let editor_state = EditorState::new(selected, content);
                                screen = Screen::Editor(editor_state);
                                execute!(std::io::stdout(), cursor::Show).ok();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if selected_index > 0 { selected_index -= 1; }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if selected_index + 1 < profiles.len() { selected_index += 1; }
                            }
                            KeyCode::Enter => {
                                if profiles.is_empty() { continue; }
                                let selected = profiles[selected_index].clone();
                                let msg = if active_vpns.contains(&selected) {
                                    toggle_vpn(&selected, "down")
                                } else {
                                    toggle_vpn(&selected, "up")
                                };
                                status_log.push(msg.clone());
                                log_status(&msg);
                            }
                            _ => {}
                        }
                    }
                }
                Screen::Status => {
                    if let Event::Key(key) = ev {
                        match key.code {
                            KeyCode::Char('w') => screen = Screen::Manager,
                            KeyCode::Char('q') => break,
                            _ => {}
                        }
                    }
                }
                Screen::Help => {
                    if let Event::Key(_) = ev { screen = Screen::Manager; }
                }
                Screen::Details { .. } => {
                    if let Event::Key(_) = ev { screen = Screen::Manager; }
                }
                Screen::Editor(editor_state) => {
                    if let Event::Key(key) = ev {
                        if let Some(result) = editor_state.handle_event(key) {
                            if result == "saved" {
                                let filename = format!("{}{}.conf", config_path(), editor_state.profile);
                                if let Err(e) = fs::write(&filename, editor_state.lines.join("\n")) {
                                    let err_msg = format!("Error saving file {}: {}", filename, e);
                                    status_log.push(err_msg.clone());
                                    log_status(&err_msg);
                                } else {
                                    let msg = format!("Updated config for {}", editor_state.profile);
                                    status_log.push(msg.clone());
                                    log_status(&msg);
                                }
                            }
                            screen = Screen::Manager;
                            execute!(std::io::stdout(), cursor::Hide).ok();
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    Ok(())
}
