use std::process::Command;
use std::fs;
use ratatui::{backend::CrosstermBackend, Terminal, widgets::{Block, Borders, List, ListItem, Paragraph}, layout::{Layout, Constraint, Direction}, style::{Style, Color, Modifier}};
use crossterm::{event::{self, KeyCode}, execute, terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};

fn list_vpn_profiles() -> Vec<String> {
    let path = "/etc/wireguard/";
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

fn toggle_vpn(profile: &str, action: &str) -> String {
    let output = Command::new("sudo")
        .arg("wg-quick")
        .arg(action)
        .arg(profile)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        format!("✅ {} VPN {} successfully\n{}", profile, action, stdout)
    } else {
        format!("❌ Failed to {} VPN: {}\n{}", action, profile, stderr)
    }
}

fn get_active_vpns() -> Vec<String> {
    let output = Command::new("wg")
        .arg("show")
        .output()
        .expect("Failed to get VPN status");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut active_vpns = Vec::new();
    
    for line in stdout.lines() {
        if line.contains("interface:") {
            let interface = line.split_whitespace().nth(1).unwrap_or("").to_string();
            active_vpns.push(interface);
        }
    }
    active_vpns
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let profiles = list_vpn_profiles();
    let mut selected_index = 0;
    let mut status_log: Vec<String> = Vec::new();
    let mut active_screen = "manager"; // "manager" or "status"

    loop {
        let active_vpns = get_active_vpns();
        if active_screen == "manager" {
            let items: Vec<ListItem> = profiles.iter()
                .enumerate()
                .map(|(i, p)| {
                    let is_active = active_vpns.contains(p);
                    let style = if i == selected_index {
                        if is_active {
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD | Modifier::REVERSED)
                        } else {
                            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
                        }
                    } else if is_active {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    };
                    ListItem::new(p.clone()).style(style)
                }).collect();
            
            terminal.draw(|f| {
                let area = f.area();
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                        Constraint::Percentage(10),
                        Constraint::Percentage(10),
                    ]).split(area);

                let block = Block::default().title(" WireGuard Manager (Press S for Status) ").borders(Borders::ALL);
                let list = List::new(items.clone()).block(block);
                f.render_widget(list, layout[0]);

                let active_connections = Paragraph::new(active_vpns.join(", "))
                    .block(Block::default().borders(Borders::ALL).title(" Active Connections "));
                f.render_widget(active_connections, layout[1]);

                let instructions = Paragraph::new("Use ↑/↓/h/j/k/l to navigate, Enter to connect/disconnect, W for Manager, S for Status, Q to quit")
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(instructions, layout[2]);

                let latest_status = status_log.last().cloned().unwrap_or_else(|| "No actions yet".to_string());
                let status = Paragraph::new(latest_status)
                    .block(Block::default().borders(Borders::ALL).title(" Latest Status "));
                f.render_widget(status, layout[3]);
            })?;
        } else if active_screen == "status" {
            let log_items: Vec<ListItem> = status_log.iter().map(|entry| ListItem::new(entry.clone())).collect();

            terminal.draw(|f| {
                let area = f.area();
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(90),
                        Constraint::Percentage(10),
                    ]).split(area);

                let block = Block::default().title(" Status Log (Press W for Manager) ").borders(Borders::ALL);
                let list = List::new(log_items).block(block);
                f.render_widget(list, layout[0]);

                let instructions = Paragraph::new("Press W to return to WireGuard Manager, Q to quit")
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(instructions, layout[1]);
            })?;
        }

        if let Ok(true) = event::poll(std::time::Duration::from_millis(200)) {
            if let event::Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => {
                        active_screen = "status";
                    }
                    KeyCode::Char('w') => {
                        active_screen = "manager";
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if active_screen == "manager" && selected_index > 0 {
                            selected_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if active_screen == "manager" && selected_index < profiles.len() - 1 {
                            selected_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if active_screen == "manager" {
                            let selected_vpn = &profiles[selected_index];
                            let message = if active_vpns.contains(selected_vpn) {
                                toggle_vpn(selected_vpn, "down")
                            } else {
                                toggle_vpn(selected_vpn, "up")
                            };
                            status_log.push(message.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
