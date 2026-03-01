use crate::app::{App, AppMode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    if size.width < 40 || size.height < 10 {
        let msg = Paragraph::new("터미널 크기가 너무 작습니다 (최소 40x10)")
            .style(Style::default().fg(Color::Red));
        frame.render_widget(msg, size);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(size);

    render_file_list(frame, app, chunks[0]);
    render_status_bar(frame, app, chunks[1]);

    match &app.mode {
        AppMode::Normal => render_info_bar(frame, app, chunks[2]),
        AppMode::Input { prompt, buffer, cursor_pos, .. } => {
            render_input_bar(frame, prompt, buffer, *cursor_pos, chunks[2]);
        }
        AppMode::Confirm { message } => {
            render_confirm_bar(frame, message, chunks[2]);
        }
    }
}

fn render_file_list(frame: &mut Frame, app: &App, area: Rect) {
    let inner_height = area.height.saturating_sub(2) as usize;
    if inner_height == 0 || app.entries.is_empty() {
        let block = Block::default()
            .title(truncate_path(&app.current_dir.to_string_lossy(), area.width as usize - 2))
            .borders(Borders::ALL);
        frame.render_widget(block, area);
        return;
    }

    let rows_per_col = inner_height;
    let col_constraints: Vec<Constraint> = (0..app.columns)
        .map(|_| Constraint::Ratio(1, app.columns as u32))
        .collect();

    let block = Block::default()
        .title(truncate_path(&app.current_dir.to_string_lossy(), area.width as usize - 2))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let col_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(col_constraints)
        .split(inner);

    let total_visible = rows_per_col * app.columns;
    let page = app.cursor / total_visible;
    let page_start = page * total_visible;

    for col in 0..app.columns {
        let col_start = page_start + col * rows_per_col;
        let mut lines: Vec<Line> = Vec::new();

        for row in 0..rows_per_col {
            let idx = col_start + row;
            if idx >= app.entries.len() {
                lines.push(Line::from(""));
                continue;
            }

            let entry = &app.entries[idx];
            let col_width = col_areas[col].width as usize;
            let is_selected = app.selected_indices.contains(&idx);

            let name_display = format_entry_name(entry, col_width, is_selected);

            let style = if idx == app.cursor {
                Style::default()
                    .bg(Color::White)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                entry_color(entry)
            };

            lines.push(Line::from(Span::styled(name_display, style)));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, col_areas[col]);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let selected_info = if app.selected_count() > 0 {
        format!(" [{}개 선택]", app.selected_count())
    } else {
        String::new()
    };

    let info = if let Some(entry) = app.selected_entry() {
        format!(
            " {} │ {} │ {} │ {}{}",
            entry.name,
            entry.display_size(),
            entry.display_date(),
            entry.display_permissions(),
            selected_info
        )
    } else {
        selected_info
    };

    let bar = Paragraph::new(Line::from(info))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(bar, area);
}

fn render_info_bar(frame: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(err) = &app.error_message {
        format!(" ⚠ {}", err)
    } else {
        let disk = match app.disk_usage() {
            Some((used, total, percent)) => {
                format!(" │ 디스크: {}/{} ({}%)", format_bytes(used), format_bytes(total), percent)
            }
            None => String::new(),
        };
        format!(
            " Dir:{} File:{}{} │ C M D R K Q",
            app.dir_count(),
            app.file_count(),
            disk
        )
    };

    let style = if app.error_message.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let bar = Paragraph::new(Line::from(text)).style(style);
    frame.render_widget(bar, area);
}

fn render_input_bar(frame: &mut Frame, prompt: &str, buffer: &str, cursor_pos: usize, area: Rect) {
    let prefix = format!(" {} ", prompt);
    let before = &buffer[..cursor_pos];
    let cursor_char = if cursor_pos < buffer.len() {
        &buffer[cursor_pos..cursor_pos + 1]
    } else {
        " "
    };
    let after = if cursor_pos < buffer.len() {
        &buffer[cursor_pos + 1..]
    } else {
        ""
    };

    let base_style = Style::default().bg(Color::Blue).fg(Color::White);
    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);

    let line = Line::from(vec![
        Span::styled(prefix, base_style),
        Span::styled(before.to_string(), base_style),
        Span::styled(cursor_char.to_string(), cursor_style),
        Span::styled(after.to_string(), base_style),
    ]);
    let bar = Paragraph::new(line).style(base_style);
    frame.render_widget(bar, area);
}

fn entry_color(entry: &crate::file_entry::FileEntry) -> Style {
    if entry.is_dir() {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if entry.is_symlink() {
        Style::default().fg(Color::Magenta)
    } else if entry.is_archive() {
        Style::default().fg(Color::Yellow)
    } else if entry.is_executable() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::White)
    }
}

fn render_confirm_bar(frame: &mut Frame, message: &str, area: Rect) {
    let text = format!(" {}", message);
    let bar = Paragraph::new(Line::from(text))
        .style(Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD));
    frame.render_widget(bar, area);
}

fn format_entry_name(entry: &crate::file_entry::FileEntry, max_width: usize, selected: bool) -> String {
    let size_str = entry.display_size();
    let size_col = 8;
    let marker = if selected { "*" } else { " " };
    let name_max = max_width.saturating_sub(size_col + 2); // +2 for marker + space

    let name = if entry.is_parent {
        "..".to_string()
    } else {
        entry.name.clone()
    };

    let truncated = if name.len() > name_max && name_max > 3 {
        format!("{}...", &name[..name_max - 3])
    } else {
        name.clone()
    };

    format!("{}{:<width$} {:>size_w$}", marker, truncated, size_str, width = name_max, size_w = size_col)
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_099_511_627_776 {
        format!("{:.1}T", bytes as f64 / 1_099_511_627_776.0)
    } else if bytes >= 1_073_741_824 {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0}M", bytes as f64 / 1_048_576.0)
    } else {
        format!("{}K", bytes / 1024)
    }
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        format!(" {} ", path)
    } else if max_len > 6 {
        format!(" ...{} ", &path[path.len() - (max_len - 4)..])
    } else {
        " ... ".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_path_short() {
        let result = truncate_path("/home/user", 50);
        assert!(result.contains("/home/user"));
    }

    #[test]
    fn test_truncate_path_long() {
        let long_path = "/very/long/path/that/exceeds/the/maximum/width/limit";
        let result = truncate_path(long_path, 20);
        assert!(result.contains("..."));
        assert!(result.len() <= 24);
    }

    #[test]
    fn test_format_entry_name_normal() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let entry = FileEntry {
            name: "test.txt".to_string(),
            path: PathBuf::from("test.txt"),
            entry_type: EntryType::File,
            size: 1024,
            modified: None,
            is_parent: false,
        };
        let formatted = format_entry_name(&entry, 30, false);
        assert!(formatted.contains("test.txt"));
        assert!(formatted.contains("1K"));
        assert!(formatted.starts_with(' ')); // 비선택 마커
    }

    #[test]
    fn test_format_entry_name_selected() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let entry = FileEntry {
            name: "test.txt".to_string(),
            path: PathBuf::from("test.txt"),
            entry_type: EntryType::File,
            size: 1024,
            modified: None,
            is_parent: false,
        };
        let formatted = format_entry_name(&entry, 30, true);
        assert!(formatted.starts_with('*')); // 선택 마커
    }

    #[test]
    fn test_format_entry_name_truncation() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let entry = FileEntry {
            name: "very_long_filename_that_should_be_truncated.txt".to_string(),
            path: PathBuf::from("very_long_filename_that_should_be_truncated.txt"),
            entry_type: EntryType::File,
            size: 500,
            modified: None,
            is_parent: false,
        };
        let formatted = format_entry_name(&entry, 25, false);
        assert!(formatted.contains("..."));
    }

    // --- Phase 3 테스트 ---

    #[test]
    fn test_entry_color_directory() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let dir = FileEntry {
            name: "src".to_string(),
            path: PathBuf::from("src"),
            entry_type: EntryType::Directory,
            size: 0,
            modified: None,
            is_parent: false,
        };
        let style = entry_color(&dir);
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_entry_color_symlink() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let link = FileEntry {
            name: "link".to_string(),
            path: PathBuf::from("link"),
            entry_type: EntryType::Symlink,
            size: 0,
            modified: None,
            is_parent: false,
        };
        let style = entry_color(&link);
        assert_eq!(style.fg, Some(Color::Magenta));
    }

    #[test]
    fn test_entry_color_archive() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let archive = FileEntry {
            name: "backup.tar.gz".to_string(),
            path: PathBuf::from("backup.tar.gz"),
            entry_type: EntryType::File,
            size: 1000,
            modified: None,
            is_parent: false,
        };
        let style = entry_color(&archive);
        assert_eq!(style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_entry_color_normal_file() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        let file = FileEntry {
            name: "readme.txt".to_string(),
            path: PathBuf::from("readme.txt"),
            entry_type: EntryType::File,
            size: 100,
            modified: None,
            is_parent: false,
        };
        let style = entry_color(&file);
        assert_eq!(style.fg, Some(Color::White));
    }
}
