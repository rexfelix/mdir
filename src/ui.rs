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

    // 뷰어 모드는 전체 화면 사용
    if app.mode == AppMode::Viewer || matches!(app.mode, AppMode::ViewerSearch { .. }) {
        render_viewer(frame, app, size);
        return;
    }

    // 도움말 모드도 전체 화면 사용
    if matches!(app.mode, AppMode::Help { .. }) {
        render_help(frame, app, size);
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
        _ => {}
    }
}

fn render_file_list(frame: &mut Frame, app: &App, area: Rect) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let title = if app.search_results {
        format!(" 검색 결과: {}개 ", app.entries.len())
    } else {
        truncate_path(&app.current_dir.to_string_lossy(), area.width as usize - 2)
    };
    if inner_height == 0 || app.entries.is_empty() {
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL);
        frame.render_widget(block, area);
        return;
    }

    let rows_per_col = inner_height;
    let col_constraints: Vec<Constraint> = (0..app.columns)
        .map(|_| Constraint::Ratio(1, app.columns as u32))
        .collect();

    let block = Block::default()
        .title(title)
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
    } else if app.search_results {
        " Enter:이동 │ Backspace:검색종료 │ Q:종료".to_string()
    } else {
        let disk = match app.disk_usage() {
            Some((used, total, percent)) => {
                format!(" │ 디스크: {}/{} ({}%)", format_bytes(used), format_bytes(total), percent)
            }
            None => String::new(),
        };
        format!(
            " Dir:{} File:{}{} │ ? V F N C M D R K Q",
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
    // cursor_pos는 char 단위이므로 char 기반으로 분할
    let char_count = buffer.chars().count();
    let before: String = buffer.chars().take(cursor_pos).collect();
    let cursor_char: String = if cursor_pos < char_count {
        buffer.chars().nth(cursor_pos).map(|c| c.to_string()).unwrap_or_else(|| " ".to_string())
    } else {
        " ".to_string()
    };
    let after: String = if cursor_pos < char_count {
        buffer.chars().skip(cursor_pos + 1).collect()
    } else {
        String::new()
    };

    let base_style = Style::default().bg(Color::Blue).fg(Color::White);
    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);

    let line = Line::from(vec![
        Span::styled(prefix, base_style),
        Span::styled(before, base_style),
        Span::styled(cursor_char, cursor_style),
        Span::styled(after, base_style),
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

fn render_viewer(frame: &mut Frame, app: &App, area: Rect) {
    let viewer = match &app.viewer {
        Some(v) => v,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    // 파일 내용 영역
    let title = format!(" {} ({} lines) ", viewer.filename, viewer.lines.len());
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let visible_height = inner.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for i in 0..visible_height {
        let line_idx = viewer.scroll + i;
        if line_idx >= viewer.lines.len() {
            lines.push(Line::from("~"));
            continue;
        }

        let line_text = &viewer.lines[line_idx];
        let is_match = viewer.search_matches.contains(&line_idx);
        let is_current_match = !viewer.search_matches.is_empty()
            && viewer.current_match < viewer.search_matches.len()
            && viewer.search_matches[viewer.current_match] == line_idx;

        let line_num = format!("{:>4} ", line_idx + 1);

        if is_current_match {
            lines.push(Line::from(vec![
                Span::styled(
                    line_num,
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    line_text.to_string(),
                    Style::default().bg(Color::Yellow).fg(Color::Black),
                ),
            ]));
        } else if is_match {
            lines.push(Line::from(vec![
                Span::styled(
                    line_num,
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    line_text.to_string(),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    line_num,
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(line_text.to_string(), Style::default().fg(Color::White)),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    // 하단바: 뷰어 정보 또는 검색 입력
    match &app.mode {
        AppMode::ViewerSearch { buffer, cursor_pos } => {
            render_input_bar(frame, "/", buffer, *cursor_pos, chunks[1]);
        }
        _ => {
            let search_info = if let Some(q) = &viewer.search_query {
                if viewer.search_matches.is_empty() {
                    format!(" │ '{}' 매치 없음", q)
                } else {
                    format!(
                        " │ '{}' {}/{}",
                        q,
                        viewer.current_match + 1,
                        viewer.search_matches.len()
                    )
                }
            } else {
                String::new()
            };
            let percent = if viewer.lines.is_empty() {
                0
            } else {
                (viewer.scroll * 100) / viewer.lines.len().max(1)
            };
            let text = format!(
                " L{}/{} ({}%){} │ ↑↓ PgUp/Dn / n/N Q",
                viewer.scroll + 1,
                viewer.lines.len(),
                percent,
                search_info
            );
            let bar = Paragraph::new(Line::from(text))
                .style(Style::default().bg(Color::DarkGray).fg(Color::White));
            frame.render_widget(bar, chunks[1]);
        }
    }
}

fn render_help(frame: &mut Frame, app: &App, area: Rect) {
    let scroll = if let AppMode::Help { scroll } = &app.mode {
        *scroll
    } else {
        return;
    };

    let help_lines = crate::app::generate_help_lines();
    let total = help_lines.len();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    let block = Block::default()
        .title(" mdir 도움말 ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let visible_height = inner.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    for i in 0..visible_height {
        let line_idx = scroll + i;
        if line_idx >= total {
            lines.push(Line::from(""));
            continue;
        }

        let text = &help_lines[line_idx];

        // 섹션 제목: [대괄호]로 시작하는 줄
        if text.trim_start().starts_with('[') && text.contains(']') {
            lines.push(Line::from(Span::styled(
                text.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
        }
        // 박스 테두리 줄
        else if text.contains('╔') || text.contains('╚') || text.contains('║') {
            lines.push(Line::from(Span::styled(
                text.to_string(),
                Style::default().fg(Color::Yellow),
            )));
        }
        // 키 설명 줄: 4칸 이상 들여쓰기 + 영문/기호로 시작
        else if text.starts_with("    ") && !text.trim().is_empty() {
            let trimmed = &text[4..];
            // 키 부분과 설명 부분 분리: 첫 번째 공백 여러 개가 나오는 지점
            if let Some(sep_pos) = find_key_desc_separator(trimmed) {
                let key_part = &text[..4 + sep_pos];
                let desc_part = &text[4 + sep_pos..];
                lines.push(Line::from(vec![
                    Span::styled(
                        key_part.to_string(),
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(
                        desc_part.to_string(),
                        Style::default().fg(Color::White),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Green),
                )));
            }
        }
        // * 참고 줄
        else if text.trim_start().starts_with('*') {
            lines.push(Line::from(Span::styled(
                text.to_string(),
                Style::default().fg(Color::DarkGray),
            )));
        }
        // 기타 줄 (빈 줄 포함)
        else {
            lines.push(Line::from(Span::styled(
                text.to_string(),
                Style::default().fg(Color::White),
            )));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    // 하단바
    let percent = if total == 0 {
        0
    } else {
        (scroll * 100) / total.max(1)
    };
    let text = format!(
        " {}/{} ({}%) │ ↑↓ PgUp/Dn │ Q/Esc/? 닫기",
        scroll + 1,
        total,
        percent
    );
    let bar = Paragraph::new(Line::from(text))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(bar, chunks[1]);
}

/// 키 이름과 설명 사이의 구분 위치를 찾는다.
/// "↑ / ↓              커서를..." 에서 연속 공백 2개 이상이 나오는 첫 위치를 반환.
fn find_key_desc_separator(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    // 먼저 비공백 문자를 건너뛴다 (키 이름 부분)
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    while i < bytes.len() && bytes[i] != b' ' {
        i += 1;
    }
    // 이후 연속 공백 2개 이상을 찾는다
    let mut space_start = i;
    let mut space_count = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            if space_count == 0 {
                space_start = i;
            }
            space_count += 1;
        } else {
            if space_count >= 2 {
                return Some(space_start);
            }
            space_count = 0;
        }
        i += 1;
    }
    None
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

    let truncated = if name.chars().count() > name_max && name_max > 3 {
        let prefix: String = name.chars().take(name_max - 3).collect();
        format!("{}...", prefix)
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

    #[test]
    fn test_format_entry_name_multibyte_truncation() {
        use crate::file_entry::{EntryType, FileEntry};
        use std::path::PathBuf;

        // 한글 파일명이 절삭되어도 panic 없이 동작해야 한다
        let entry = FileEntry {
            name: "한글파일이름이_매우_긴_경우입니다.txt".to_string(),
            path: PathBuf::from("한글파일이름이_매우_긴_경우입니다.txt"),
            entry_type: EntryType::File,
            size: 100,
            modified: None,
            is_parent: false,
        };
        let formatted = format_entry_name(&entry, 20, false);
        assert!(formatted.contains("..."));
        // panic 없이 정상 실행되면 성공
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
