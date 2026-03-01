use crate::event::KeyAction;
use crate::file_entry::{self, FileEntry};
use std::path::PathBuf;

pub struct App {
    pub current_dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub columns: usize,
    pub rows_per_col: usize,
    pub terminal_width: u16,
    pub terminal_height: u16,
    pub show_hidden: bool,
    pub should_quit: bool,
    pub error_message: Option<String>,
    previous_dir: Option<String>,
}

impl App {
    pub fn new(path: PathBuf) -> Self {
        let mut app = Self {
            current_dir: path,
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            columns: 2,
            rows_per_col: 20,
            terminal_width: 80,
            terminal_height: 24,
            show_hidden: false,
            should_quit: false,
            error_message: None,
            previous_dir: None,
        };
        app.load_directory();
        app
    }

    pub fn load_directory(&mut self) {
        match file_entry::read_directory(&self.current_dir, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                self.error_message = None;

                if let Some(prev) = &self.previous_dir {
                    self.cursor = self
                        .entries
                        .iter()
                        .position(|e| e.name == *prev)
                        .unwrap_or(0);
                } else {
                    self.cursor = 0;
                }
                self.previous_dir = None;
                self.clamp_cursor();
            }
            Err(e) => {
                self.error_message = Some(format!("디렉토리 읽기 실패: {}", e));
            }
        }
    }

    pub fn update_layout(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;
        self.columns = calculate_columns(width);
        // 상태바 2줄 차감
        let available_height = height.saturating_sub(3) as usize;
        self.rows_per_col = if available_height == 0 {
            1
        } else {
            available_height
        };
        self.clamp_cursor();
    }

    pub fn handle_key(&mut self, action: KeyAction) {
        self.error_message = None;
        match action {
            KeyAction::MoveUp => self.move_up(),
            KeyAction::MoveDown => self.move_down(),
            KeyAction::MoveLeft => self.move_left(),
            KeyAction::MoveRight => self.move_right(),
            KeyAction::Home => self.move_home(),
            KeyAction::End => self.move_end(),
            KeyAction::PageUp => self.page_up(),
            KeyAction::PageDown => self.page_down(),
            KeyAction::Enter => self.enter(),
            KeyAction::Backspace => self.go_parent(),
            KeyAction::ToggleHidden => self.toggle_hidden(),
            KeyAction::Quit => self.should_quit = true,
            KeyAction::Noop => {}
        }
    }

    fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
        }
    }

    fn move_left(&mut self) {
        if self.cursor >= self.rows_per_col {
            self.cursor -= self.rows_per_col;
        }
    }

    fn move_right(&mut self) {
        let new_pos = self.cursor + self.rows_per_col;
        if new_pos < self.entries.len() {
            self.cursor = new_pos;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = self.entries.len() - 1;
        }
    }

    fn page_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(self.rows_per_col);
    }

    fn page_down(&mut self) {
        let new_pos = self.cursor + self.rows_per_col;
        if new_pos < self.entries.len() {
            self.cursor = new_pos;
        } else if !self.entries.is_empty() {
            self.cursor = self.entries.len() - 1;
        }
    }

    fn enter(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.is_dir() || entry.is_parent {
                let target = entry.path.clone();

                // 진입 전에 대상 디렉토리 읽기 가능 여부를 확인
                if let Err(e) = std::fs::read_dir(&target) {
                    self.error_message =
                        Some(format!("디렉토리 진입 실패: {}", e));
                    return;
                }

                if entry.is_parent {
                    self.previous_dir = Some(
                        self.current_dir
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    );
                }
                self.current_dir = target;
                self.cursor = 0;
                self.scroll_offset = 0;
                self.load_directory();
            }
        }
    }

    fn go_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.previous_dir = Some(
                self.current_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            self.current_dir = parent.to_path_buf();
            self.cursor = 0;
            self.scroll_offset = 0;
            self.load_directory();
        }
    }

    fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        let current_name = self
            .entries
            .get(self.cursor)
            .map(|e| e.name.clone());
        self.load_directory();
        if let Some(name) = current_name {
            self.cursor = self
                .entries
                .iter()
                .position(|e| e.name == name)
                .unwrap_or(0);
        }
    }

    fn clamp_cursor(&mut self) {
        if self.entries.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.entries.len() {
            self.cursor = self.entries.len() - 1;
        }
    }

    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.cursor)
    }

    pub fn dir_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.is_dir() && !e.is_parent)
            .count()
    }

    pub fn file_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_dir()).count()
    }
}

pub fn calculate_columns(width: u16) -> usize {
    if width >= 120 {
        3
    } else if width >= 80 {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_app() -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("alpha_dir")).unwrap();
        fs::create_dir(dir.path().join("beta_dir")).unwrap();
        fs::write(dir.path().join("file_a.txt"), "aaa").unwrap();
        fs::write(dir.path().join("file_b.txt"), "bbb").unwrap();
        fs::write(dir.path().join(".hidden"), "hidden").unwrap();
        let app = App::new(dir.path().to_path_buf());
        (app, dir)
    }

    #[test]
    fn test_initial_state() {
        let (app, _dir) = create_test_app();
        assert_eq!(app.cursor, 0);
        assert!(!app.should_quit);
        assert!(!app.entries.is_empty());
        // .. + 2 dirs + 2 files (hidden excluded)
        assert_eq!(app.entries.len(), 5);
    }

    #[test]
    fn test_move_down_up() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::MoveDown);
        assert_eq!(app.cursor, 1);
        app.handle_key(KeyAction::MoveDown);
        assert_eq!(app.cursor, 2);
        app.handle_key(KeyAction::MoveUp);
        assert_eq!(app.cursor, 1);
    }

    #[test]
    fn test_cursor_bounds() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::MoveUp);
        assert_eq!(app.cursor, 0); // 0 이하로 내려가지 않음

        app.handle_key(KeyAction::End);
        let last = app.entries.len() - 1;
        assert_eq!(app.cursor, last);

        app.handle_key(KeyAction::MoveDown);
        assert_eq!(app.cursor, last); // 마지막을 넘지 않음
    }

    #[test]
    fn test_home_end() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::End);
        assert_eq!(app.cursor, app.entries.len() - 1);
        app.handle_key(KeyAction::Home);
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn test_enter_directory() {
        let (mut app, _dir) = create_test_app();
        // 커서를 alpha_dir로 이동 (index 1: .. 다음)
        app.handle_key(KeyAction::MoveDown);
        assert!(app.entries[app.cursor].is_dir());

        let dir_name = app.entries[app.cursor].name.clone();
        app.handle_key(KeyAction::Enter);
        assert!(app.current_dir.ends_with(&dir_name));
    }

    #[test]
    fn test_go_parent_restores_cursor() {
        let (mut app, dir) = create_test_app();
        let sub = dir.path().join("alpha_dir");
        fs::write(sub.join("inner.txt"), "in").unwrap();

        // alpha_dir 진입
        app.handle_key(KeyAction::MoveDown);
        app.handle_key(KeyAction::Enter);
        assert!(app.current_dir.ends_with("alpha_dir"));

        // Backspace로 상위로
        app.handle_key(KeyAction::Backspace);
        assert_eq!(app.current_dir, dir.path());
        // 커서가 alpha_dir에 위치해야 함
        assert_eq!(app.entries[app.cursor].name, "alpha_dir");
    }

    #[test]
    fn test_toggle_hidden() {
        let (mut app, _dir) = create_test_app();
        let count_before = app.entries.len();
        app.handle_key(KeyAction::ToggleHidden);
        assert!(app.entries.len() > count_before);
        assert!(app.entries.iter().any(|e| e.name == ".hidden"));
    }

    #[test]
    fn test_quit() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_calculate_columns() {
        assert_eq!(calculate_columns(60), 1);
        assert_eq!(calculate_columns(80), 2);
        assert_eq!(calculate_columns(119), 2);
        assert_eq!(calculate_columns(120), 3);
        assert_eq!(calculate_columns(200), 3);
    }

    #[test]
    fn test_column_movement() {
        let (mut app, _dir) = create_test_app();
        app.rows_per_col = 2;
        // 5개 항목, rows_per_col=2이면 좌/우 이동은 ±2
        app.cursor = 0;
        app.handle_key(KeyAction::MoveRight);
        assert_eq!(app.cursor, 2);
        app.handle_key(KeyAction::MoveLeft);
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn test_dir_file_count() {
        let (app, _dir) = create_test_app();
        assert_eq!(app.dir_count(), 2);
        assert_eq!(app.file_count(), 2);
    }

    #[test]
    fn test_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(dir.path().to_path_buf());
        assert_eq!(app.entries.len(), 1); // .. 만 존재
        assert!(app.entries[0].is_parent);
    }

    #[test]
    fn test_unknown_key_no_movement() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 2;
        app.handle_key(KeyAction::Noop);
        assert_eq!(app.cursor, 2); // 커서 변경 없음
    }

    #[test]
    fn test_enter_permission_denied() {
        let dir = tempfile::tempdir().unwrap();
        let restricted = dir.path().join("restricted_dir");
        fs::create_dir(&restricted).unwrap();

        // 권한 제거
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000)).unwrap();
        }

        let mut app = App::new(dir.path().to_path_buf());
        let original_dir = app.current_dir.clone();
        let original_entries_len = app.entries.len();

        // restricted_dir로 커서 이동
        let restricted_idx = app
            .entries
            .iter()
            .position(|e| e.name == "restricted_dir")
            .unwrap();
        app.cursor = restricted_idx;

        // Enter 시도 - 권한 에러로 현재 위치 유지되어야 함
        app.handle_key(KeyAction::Enter);

        #[cfg(unix)]
        {
            assert_eq!(app.current_dir, original_dir, "current_dir이 변경되면 안 됨");
            assert_eq!(app.entries.len(), original_entries_len, "entries가 변경되면 안 됨");
            assert!(app.error_message.is_some(), "에러 메시지가 설정되어야 함");

            // 권한 복원 (정리용)
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn test_root_directory_backspace() {
        let mut app = App::new(std::path::PathBuf::from("/"));
        let original_dir = app.current_dir.clone();
        app.handle_key(KeyAction::Backspace);
        assert_eq!(app.current_dir, original_dir, "루트에서 Backspace 시 위치 유지");
    }

    #[test]
    fn test_page_up_down() {
        let (mut app, _dir) = create_test_app();
        app.rows_per_col = 2;

        // PageDown: cursor 0 → 2 (rows_per_col만큼 이동)
        app.cursor = 0;
        app.handle_key(KeyAction::PageDown);
        assert_eq!(app.cursor, 2);

        // PageDown 경계: 마지막 넘어가면 마지막 항목으로
        app.cursor = 4; // 마지막 항목
        app.handle_key(KeyAction::PageDown);
        assert_eq!(app.cursor, 4); // 마지막 유지

        // PageUp: cursor 3 → 1 (rows_per_col만큼 역이동)
        app.cursor = 3;
        app.handle_key(KeyAction::PageUp);
        assert_eq!(app.cursor, 1);

        // PageUp 경계: 0 이하로 안 내려감
        app.cursor = 0;
        app.handle_key(KeyAction::PageUp);
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn test_enter_on_file() {
        let (mut app, _dir) = create_test_app();
        // 파일 항목으로 이동 (디렉토리가 아닌 것)
        let file_idx = app
            .entries
            .iter()
            .position(|e| !e.is_dir() && !e.is_parent)
            .unwrap();
        app.cursor = file_idx;

        let original_dir = app.current_dir.clone();
        app.handle_key(KeyAction::Enter);
        assert_eq!(app.current_dir, original_dir, "파일에 Enter 시 디렉토리 변경 없음");
    }

    #[test]
    fn test_enter_parent_entry() {
        let (mut app, dir) = create_test_app();
        let sub = dir.path().join("alpha_dir");
        fs::write(sub.join("inner.txt"), "in").unwrap();

        // alpha_dir 진입
        app.handle_key(KeyAction::MoveDown);
        app.handle_key(KeyAction::Enter);

        // .. (parent) 항목에 Enter
        assert_eq!(app.cursor, 0);
        assert!(app.entries[0].is_parent);
        app.handle_key(KeyAction::Enter);

        // 상위로 이동 + 커서가 alpha_dir에 복원
        assert_eq!(app.current_dir, dir.path());
        assert_eq!(app.entries[app.cursor].name, "alpha_dir");
    }

    #[test]
    fn test_display_size_mb_gb() {
        use crate::file_entry::{EntryType, FileEntry};

        let mb_entry = FileEntry {
            name: "big.bin".to_string(),
            path: std::path::PathBuf::from("big.bin"),
            entry_type: EntryType::File,
            size: 5_242_880, // 5MB
            modified: None,
            is_parent: false,
        };
        assert_eq!(mb_entry.display_size(), "5.0M");

        let gb_entry = FileEntry {
            name: "huge.iso".to_string(),
            path: std::path::PathBuf::from("huge.iso"),
            entry_type: EntryType::File,
            size: 2_147_483_648, // 2GB
            modified: None,
            is_parent: false,
        };
        assert_eq!(gb_entry.display_size(), "2.0G");
    }

    #[test]
    fn test_update_layout() {
        let (mut app, _dir) = create_test_app();

        app.update_layout(60, 30);
        assert_eq!(app.columns, 1);
        assert_eq!(app.rows_per_col, 27); // 30 - 3

        app.update_layout(100, 40);
        assert_eq!(app.columns, 2);
        assert_eq!(app.rows_per_col, 37); // 40 - 3

        app.update_layout(150, 50);
        assert_eq!(app.columns, 3);
        assert_eq!(app.rows_per_col, 47); // 50 - 3
    }
}
