use crate::event::KeyAction;
use crate::file_entry::{self, FileEntry};
use crate::file_ops;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputPurpose {
    Copy,
    Move,
    Rename,
    Mkdir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Input {
        purpose: InputPurpose,
        buffer: String,
        prompt: String,
        cursor_pos: usize,
    },
    Confirm {
        message: String,
    },
}

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
    pub mode: AppMode,
    pub selected_indices: HashSet<usize>,
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
            mode: AppMode::Normal,
            selected_indices: HashSet::new(),
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
        let available_height = height.saturating_sub(3) as usize;
        self.rows_per_col = if available_height == 0 {
            1
        } else {
            available_height
        };
        self.clamp_cursor();
    }

    pub fn input_mode(&self) -> crate::event::InputMode {
        match &self.mode {
            AppMode::Normal => crate::event::InputMode::Normal,
            AppMode::Input { .. } => crate::event::InputMode::Input,
            AppMode::Confirm { .. } => crate::event::InputMode::Confirm,
        }
    }

    pub fn handle_key(&mut self, action: KeyAction) {
        match &self.mode {
            AppMode::Normal => self.handle_normal_key(action),
            AppMode::Input { .. } => self.handle_input_key(action),
            AppMode::Confirm { .. } => self.handle_confirm_key(action),
        }
    }

    fn handle_normal_key(&mut self, action: KeyAction) {
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
            KeyAction::Select => self.toggle_select(),
            KeyAction::Copy => self.start_copy(),
            KeyAction::Move => self.start_move(),
            KeyAction::Delete => self.start_delete(),
            KeyAction::Rename => self.start_rename(),
            KeyAction::Mkdir => self.start_mkdir(),
            KeyAction::Quit => self.should_quit = true,
            KeyAction::Noop => {}
            // 입력/확인 모드 키는 Normal에서 무시
            _ => {}
        }
    }

    fn handle_input_key(&mut self, action: KeyAction) {
        match action {
            KeyAction::InputChar(c) => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    buffer.insert(*cursor_pos, c);
                    *cursor_pos += 1;
                }
            }
            KeyAction::InputBackspace => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    if *cursor_pos > 0 {
                        buffer.remove(*cursor_pos - 1);
                        *cursor_pos -= 1;
                    }
                }
            }
            KeyAction::InputDelete => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    if *cursor_pos < buffer.len() {
                        buffer.remove(*cursor_pos);
                    }
                }
            }
            KeyAction::InputCursorLeft => {
                if let AppMode::Input { cursor_pos, .. } = &mut self.mode {
                    if *cursor_pos > 0 {
                        *cursor_pos -= 1;
                    }
                }
            }
            KeyAction::InputCursorRight => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    if *cursor_pos < buffer.len() {
                        *cursor_pos += 1;
                    }
                }
            }
            KeyAction::InputCursorHome => {
                if let AppMode::Input { cursor_pos, .. } = &mut self.mode {
                    *cursor_pos = 0;
                }
            }
            KeyAction::InputCursorEnd => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    *cursor_pos = buffer.len();
                }
            }
            KeyAction::InputConfirm => self.execute_input(),
            KeyAction::InputCancel => {
                self.mode = AppMode::Normal;
                self.error_message = None;
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, action: KeyAction) {
        match action {
            KeyAction::ConfirmYes => self.execute_delete(),
            KeyAction::ConfirmNo => {
                self.mode = AppMode::Normal;
                self.error_message = None;
            }
            _ => {}
        }
    }

    // --- 네비게이션 ---

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
                self.selected_indices.clear();
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
            self.selected_indices.clear();
            self.load_directory();
        }
    }

    fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        let current_name = self
            .entries
            .get(self.cursor)
            .map(|e| e.name.clone());
        self.selected_indices.clear();
        self.load_directory();
        if let Some(name) = current_name {
            self.cursor = self
                .entries
                .iter()
                .position(|e| e.name == name)
                .unwrap_or(0);
        }
    }

    // --- 선택 ---

    fn toggle_select(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.is_parent {
                return; // .. 는 선택 불가
            }
        }
        if self.selected_indices.contains(&self.cursor) {
            self.selected_indices.remove(&self.cursor);
        } else {
            self.selected_indices.insert(self.cursor);
        }
        // 선택 후 커서를 한 칸 아래로
        self.move_down();
    }

    /// CRUD 대상 파일 경로 목록을 반환한다.
    /// 선택된 파일이 있으면 선택 파일, 없으면 커서 파일.
    fn target_paths(&self) -> Vec<PathBuf> {
        if self.selected_indices.is_empty() {
            if let Some(entry) = self.entries.get(self.cursor) {
                if !entry.is_parent {
                    return vec![entry.path.clone()];
                }
            }
            Vec::new()
        } else {
            self.selected_indices
                .iter()
                .filter_map(|&idx| self.entries.get(idx))
                .filter(|e| !e.is_parent)
                .map(|e| e.path.clone())
                .collect()
        }
    }

    fn target_count(&self) -> usize {
        if self.selected_indices.is_empty() {
            if let Some(entry) = self.entries.get(self.cursor) {
                if !entry.is_parent {
                    return 1;
                }
            }
            0
        } else {
            self.selected_indices.len()
        }
    }

    // --- CRUD 진입 ---

    fn start_copy(&mut self) {
        if self.target_count() == 0 {
            self.error_message = Some("복사할 파일이 없습니다".to_string());
            return;
        }
        let default = self.current_dir.to_string_lossy().to_string();
        let len = default.len();
        self.mode = AppMode::Input {
            purpose: InputPurpose::Copy,
            buffer: default,
            prompt: format!("복사 대상 경로 ({}개 파일):", self.target_count()),
            cursor_pos: len,
        };
    }

    fn start_move(&mut self) {
        if self.target_count() == 0 {
            self.error_message = Some("이동할 파일이 없습니다".to_string());
            return;
        }
        let default = self.current_dir.to_string_lossy().to_string();
        let len = default.len();
        self.mode = AppMode::Input {
            purpose: InputPurpose::Move,
            buffer: default,
            prompt: format!("이동 대상 경로 ({}개 파일):", self.target_count()),
            cursor_pos: len,
        };
    }

    fn start_delete(&mut self) {
        let count = self.target_count();
        if count == 0 {
            self.error_message = Some("삭제할 파일이 없습니다".to_string());
            return;
        }
        let names: Vec<String> = if self.selected_indices.is_empty() {
            self.entries
                .get(self.cursor)
                .map(|e| vec![e.name.clone()])
                .unwrap_or_default()
        } else {
            self.selected_indices
                .iter()
                .filter_map(|&idx| self.entries.get(idx))
                .map(|e| e.name.clone())
                .collect()
        };
        let display = if names.len() <= 3 {
            names.join(", ")
        } else {
            format!("{} 외 {}개", names[0], names.len() - 1)
        };
        self.mode = AppMode::Confirm {
            message: format!("삭제하시겠습니까? [{}] (Y/N)", display),
        };
    }

    fn start_rename(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.is_parent {
                self.error_message = Some("'..'은 이름 변경할 수 없습니다".to_string());
                return;
            }
            let len = entry.name.len();
            self.mode = AppMode::Input {
                purpose: InputPurpose::Rename,
                buffer: entry.name.clone(),
                prompt: format!("새 이름 ({}→):", entry.name),
                cursor_pos: len,
            };
        }
    }

    fn start_mkdir(&mut self) {
        self.mode = AppMode::Input {
            purpose: InputPurpose::Mkdir,
            buffer: String::new(),
            prompt: "새 디렉토리명:".to_string(),
            cursor_pos: 0,
        };
    }

    // --- CRUD 실행 ---

    fn execute_input(&mut self) {
        let mode = self.mode.clone();
        if let AppMode::Input { purpose, buffer, .. } = mode {
            let result = match purpose {
                InputPurpose::Copy => self.exec_copy(&buffer),
                InputPurpose::Move => self.exec_move(&buffer),
                InputPurpose::Rename => self.exec_rename(&buffer),
                InputPurpose::Mkdir => self.exec_mkdir(&buffer),
            };
            self.mode = AppMode::Normal;
            if let Err(e) = result {
                self.error_message = Some(e);
            } else {
                self.selected_indices.clear();
                self.load_directory();
            }
        }
    }

    fn exec_copy(&self, dest: &str) -> Result<(), String> {
        let dest_path = PathBuf::from(dest);
        let paths = self.target_paths();
        let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        file_ops::copy_entries(&refs, &dest_path)
    }

    fn exec_move(&self, dest: &str) -> Result<(), String> {
        let dest_path = PathBuf::from(dest);
        let paths = self.target_paths();
        let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        file_ops::move_entries(&refs, &dest_path)
    }

    fn exec_rename(&self, new_name: &str) -> Result<(), String> {
        if let Some(entry) = self.entries.get(self.cursor) {
            file_ops::rename_entry(&entry.path, new_name)
        } else {
            Err("이름 변경 대상이 없습니다".to_string())
        }
    }

    fn exec_mkdir(&self, name: &str) -> Result<(), String> {
        file_ops::create_directory(&self.current_dir, name)
    }

    fn execute_delete(&mut self) {
        let paths = self.target_paths();
        let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        match file_ops::delete_entries(&refs) {
            Ok(()) => {
                self.selected_indices.clear();
                self.load_directory();
            }
            Err(e) => {
                self.error_message = Some(e);
            }
        }
        self.mode = AppMode::Normal;
    }

    // --- 유틸 ---

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

    pub fn selected_count(&self) -> usize {
        self.selected_indices.len()
    }

    /// 현재 디렉토리의 디스크 사용량을 반환한다.
    /// (사용량 바이트, 전체 바이트, 사용률 퍼센트)
    pub fn disk_usage(&self) -> Option<(u64, u64, u8)> {
        disk_usage_for_path(&self.current_dir)
    }
}

#[cfg(unix)]
fn disk_usage_for_path(path: &std::path::Path) -> Option<(u64, u64, u8)> {
    use std::ffi::CString;
    let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
            let block_size = stat.f_frsize as u64;
            let total = stat.f_blocks as u64 * block_size;
            let available = stat.f_bavail as u64 * block_size;
            let used = total.saturating_sub(available);
            let percent = if total > 0 {
                (used as f64 / total as f64 * 100.0) as u8
            } else {
                0
            };
            Some((used, total, percent))
        } else {
            None
        }
    }
}

#[cfg(not(unix))]
fn disk_usage_for_path(_path: &std::path::Path) -> Option<(u64, u64, u8)> {
    None
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

    // --- Phase 1 기존 테스트 ---

    #[test]
    fn test_initial_state() {
        let (app, _dir) = create_test_app();
        assert_eq!(app.cursor, 0);
        assert!(!app.should_quit);
        assert!(!app.entries.is_empty());
        assert_eq!(app.entries.len(), 5);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.selected_indices.is_empty());
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
        assert_eq!(app.cursor, 0);

        app.handle_key(KeyAction::End);
        let last = app.entries.len() - 1;
        assert_eq!(app.cursor, last);

        app.handle_key(KeyAction::MoveDown);
        assert_eq!(app.cursor, last);
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

        app.handle_key(KeyAction::MoveDown);
        app.handle_key(KeyAction::Enter);
        assert!(app.current_dir.ends_with("alpha_dir"));

        app.handle_key(KeyAction::Backspace);
        assert_eq!(app.current_dir, dir.path());
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
        assert_eq!(app.entries.len(), 1);
        assert!(app.entries[0].is_parent);
    }

    #[test]
    fn test_unknown_key_no_movement() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 2;
        app.handle_key(KeyAction::Noop);
        assert_eq!(app.cursor, 2);
    }

    #[test]
    fn test_enter_permission_denied() {
        let dir = tempfile::tempdir().unwrap();
        let restricted = dir.path().join("restricted_dir");
        fs::create_dir(&restricted).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&restricted, fs::Permissions::from_mode(0o000)).unwrap();
        }

        let mut app = App::new(dir.path().to_path_buf());
        let original_dir = app.current_dir.clone();
        let original_entries_len = app.entries.len();

        let restricted_idx = app
            .entries
            .iter()
            .position(|e| e.name == "restricted_dir")
            .unwrap();
        app.cursor = restricted_idx;

        app.handle_key(KeyAction::Enter);

        #[cfg(unix)]
        {
            assert_eq!(app.current_dir, original_dir);
            assert_eq!(app.entries.len(), original_entries_len);
            assert!(app.error_message.is_some());

            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&restricted, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn test_root_directory_backspace() {
        let mut app = App::new(std::path::PathBuf::from("/"));
        let original_dir = app.current_dir.clone();
        app.handle_key(KeyAction::Backspace);
        assert_eq!(app.current_dir, original_dir);
    }

    #[test]
    fn test_page_up_down() {
        let (mut app, _dir) = create_test_app();
        app.rows_per_col = 2;

        app.cursor = 0;
        app.handle_key(KeyAction::PageDown);
        assert_eq!(app.cursor, 2);

        app.cursor = 4;
        app.handle_key(KeyAction::PageDown);
        assert_eq!(app.cursor, 4);

        app.cursor = 3;
        app.handle_key(KeyAction::PageUp);
        assert_eq!(app.cursor, 1);

        app.cursor = 0;
        app.handle_key(KeyAction::PageUp);
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn test_enter_on_file() {
        let (mut app, _dir) = create_test_app();
        let file_idx = app
            .entries
            .iter()
            .position(|e| !e.is_dir() && !e.is_parent)
            .unwrap();
        app.cursor = file_idx;

        let original_dir = app.current_dir.clone();
        app.handle_key(KeyAction::Enter);
        assert_eq!(app.current_dir, original_dir);
    }

    #[test]
    fn test_enter_parent_entry() {
        let (mut app, dir) = create_test_app();
        let sub = dir.path().join("alpha_dir");
        fs::write(sub.join("inner.txt"), "in").unwrap();

        app.handle_key(KeyAction::MoveDown);
        app.handle_key(KeyAction::Enter);

        assert_eq!(app.cursor, 0);
        assert!(app.entries[0].is_parent);
        app.handle_key(KeyAction::Enter);

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
            size: 5_242_880,
            modified: None,
            is_parent: false,
        };
        assert_eq!(mb_entry.display_size(), "5.0M");

        let gb_entry = FileEntry {
            name: "huge.iso".to_string(),
            path: std::path::PathBuf::from("huge.iso"),
            entry_type: EntryType::File,
            size: 2_147_483_648,
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
        assert_eq!(app.rows_per_col, 27);

        app.update_layout(100, 40);
        assert_eq!(app.columns, 2);
        assert_eq!(app.rows_per_col, 37);

        app.update_layout(150, 50);
        assert_eq!(app.columns, 3);
        assert_eq!(app.rows_per_col, 47);
    }

    // --- Phase 2 선택 테스트 ---

    #[test]
    fn test_select_toggle() {
        let (mut app, _dir) = create_test_app();
        // 커서를 첫 번째 디렉토리로 이동 (index 1: alpha_dir)
        app.cursor = 1;
        app.handle_key(KeyAction::Select);
        assert!(app.selected_indices.contains(&1));
        assert_eq!(app.cursor, 2); // 자동 다음 이동

        // 다시 선택 해제
        app.cursor = 1;
        app.handle_key(KeyAction::Select);
        assert!(!app.selected_indices.contains(&1));
    }

    #[test]
    fn test_select_parent_blocked() {
        let (mut app, _dir) = create_test_app();
        // cursor 0 = ".."
        app.cursor = 0;
        app.handle_key(KeyAction::Select);
        assert!(app.selected_indices.is_empty());
    }

    #[test]
    fn test_select_cleared_on_dir_change() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 1;
        app.handle_key(KeyAction::Select);
        assert!(!app.selected_indices.is_empty());

        // 디렉토리 진입 시 선택 초기화
        app.cursor = 1; // alpha_dir
        app.handle_key(KeyAction::Enter);
        assert!(app.selected_indices.is_empty());
    }

    #[test]
    fn test_selected_count() {
        let (mut app, _dir) = create_test_app();
        assert_eq!(app.selected_count(), 0);

        app.cursor = 1;
        app.handle_key(KeyAction::Select); // alpha_dir 선택
        app.handle_key(KeyAction::Select); // beta_dir 선택
        assert_eq!(app.selected_count(), 2);
    }

    // --- Phase 2 CRUD 테스트 ---

    #[test]
    fn test_start_copy_enters_input_mode() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Copy);
        match &app.mode {
            AppMode::Input { purpose, .. } => assert_eq!(*purpose, InputPurpose::Copy),
            _ => panic!("Copy 키로 Input 모드 진입 실패"),
        }
    }

    #[test]
    fn test_start_delete_enters_confirm_mode() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Delete);
        match &app.mode {
            AppMode::Confirm { message } => assert!(message.contains("삭제")),
            _ => panic!("Delete 키로 Confirm 모드 진입 실패"),
        }
    }

    #[test]
    fn test_start_rename_enters_input_mode() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Rename);
        match &app.mode {
            AppMode::Input { purpose, buffer, cursor_pos, .. } => {
                assert_eq!(*purpose, InputPurpose::Rename);
                assert_eq!(buffer, "file_a.txt");
                assert_eq!(*cursor_pos, buffer.len());
            }
            _ => panic!("Rename 키로 Input 모드 진입 실패"),
        }
    }

    #[test]
    fn test_start_mkdir_enters_input_mode() {
        let (mut app, _) = create_test_app();
        app.handle_key(KeyAction::Mkdir);
        match &app.mode {
            AppMode::Input { purpose, buffer, cursor_pos, .. } => {
                assert_eq!(*purpose, InputPurpose::Mkdir);
                assert!(buffer.is_empty());
                assert_eq!(*cursor_pos, 0);
            }
            _ => panic!("Mkdir 키로 Input 모드 진입 실패"),
        }
    }

    #[test]
    fn test_input_char_and_backspace() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::Mkdir);

        app.handle_key(KeyAction::InputChar('a'));
        app.handle_key(KeyAction::InputChar('b'));
        app.handle_key(KeyAction::InputChar('c'));

        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "abc");
            assert_eq!(*cursor_pos, 3);
        } else {
            panic!("Input 모드가 아님");
        }

        app.handle_key(KeyAction::InputBackspace);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "ab");
            assert_eq!(*cursor_pos, 2);
        }
    }

    #[test]
    fn test_input_cursor_movement() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::Mkdir);

        // "abcde" 입력
        for c in "abcde".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }

        // Left로 커서 이동
        app.handle_key(KeyAction::InputCursorLeft);
        app.handle_key(KeyAction::InputCursorLeft);
        if let AppMode::Input { cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, 3); // "abc|de"
        }

        // 중간에 문자 삽입
        app.handle_key(KeyAction::InputChar('X'));
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "abcXde");
            assert_eq!(*cursor_pos, 4);
        }

        // Backspace로 커서 앞 문자 삭제
        app.handle_key(KeyAction::InputBackspace);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "abcde");
            assert_eq!(*cursor_pos, 3);
        }

        // Delete로 커서 뒤 문자 삭제
        app.handle_key(KeyAction::InputDelete);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "abce");
            assert_eq!(*cursor_pos, 3);
        }

        // Home으로 처음으로
        app.handle_key(KeyAction::InputCursorHome);
        if let AppMode::Input { cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, 0);
        }

        // End로 끝으로
        app.handle_key(KeyAction::InputCursorEnd);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, buffer.len());
        }
    }

    #[test]
    fn test_input_cursor_boundary() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::Mkdir);

        // 빈 상태에서 Left/Backspace/Delete 안전
        app.handle_key(KeyAction::InputCursorLeft);
        app.handle_key(KeyAction::InputBackspace);
        app.handle_key(KeyAction::InputDelete);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "");
            assert_eq!(*cursor_pos, 0);
        }

        // "ab" 입력 후 End에서 Right 무시
        app.handle_key(KeyAction::InputChar('a'));
        app.handle_key(KeyAction::InputChar('b'));
        app.handle_key(KeyAction::InputCursorRight);
        if let AppMode::Input { cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, 2); // 끝에서 더 안 감
        }

        // 끝에서 Delete 무시
        app.handle_key(KeyAction::InputDelete);
        if let AppMode::Input { buffer, .. } = &app.mode {
            assert_eq!(buffer, "ab");
        }
    }

    #[test]
    fn test_input_cancel() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::Mkdir);
        assert!(matches!(app.mode, AppMode::Input { .. }));

        app.handle_key(KeyAction::InputCancel);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_confirm_no_cancels() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 3;
        app.handle_key(KeyAction::Delete);
        assert!(matches!(app.mode, AppMode::Confirm { .. }));

        app.handle_key(KeyAction::ConfirmNo);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_mkdir_execute() {
        let (mut app, dir) = create_test_app();
        app.handle_key(KeyAction::Mkdir);

        // "new_dir" 입력
        for c in "new_dir".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }
        app.handle_key(KeyAction::InputConfirm);

        assert_eq!(app.mode, AppMode::Normal);
        assert!(dir.path().join("new_dir").is_dir());
        assert!(app.entries.iter().any(|e| e.name == "new_dir"));
    }

    #[test]
    fn test_delete_execute() {
        let (mut app, dir) = create_test_app();
        // file_a.txt 선택 후 삭제
        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Delete);
        app.handle_key(KeyAction::ConfirmYes);

        assert_eq!(app.mode, AppMode::Normal);
        assert!(!dir.path().join("file_a.txt").exists());
    }

    #[test]
    fn test_rename_execute() {
        let (mut app, dir) = create_test_app();
        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Rename);

        // 기존 이름 지우고 새 이름 입력
        // buffer에는 "file_a.txt"가 들어있으므로 전체 지우고 새 이름
        // 실제로는 사용자가 수정하겠지만, 테스트에서는 직접 buffer 교체
        if let AppMode::Input { buffer, .. } = &mut app.mode {
            buffer.clear();
            buffer.push_str("renamed.txt");
        }
        app.handle_key(KeyAction::InputConfirm);

        assert_eq!(app.mode, AppMode::Normal);
        assert!(!dir.path().join("file_a.txt").exists());
        assert!(dir.path().join("renamed.txt").exists());
    }

    #[test]
    fn test_copy_execute() {
        let (mut app, dir) = create_test_app();
        let dest = tempfile::tempdir().unwrap();

        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Copy);

        // buffer를 dest 경로로 교체
        if let AppMode::Input { buffer, .. } = &mut app.mode {
            buffer.clear();
            buffer.push_str(&dest.path().to_string_lossy());
        }
        app.handle_key(KeyAction::InputConfirm);

        assert_eq!(app.mode, AppMode::Normal);
        assert!(dir.path().join("file_a.txt").exists()); // 원본 유지
        assert!(dest.path().join("file_a.txt").exists()); // 복사됨
    }

    #[test]
    fn test_move_execute() {
        let (mut app, dir) = create_test_app();
        let dest = tempfile::tempdir().unwrap();

        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Move);

        if let AppMode::Input { buffer, .. } = &mut app.mode {
            buffer.clear();
            buffer.push_str(&dest.path().to_string_lossy());
        }
        app.handle_key(KeyAction::InputConfirm);

        assert_eq!(app.mode, AppMode::Normal);
        assert!(!dir.path().join("file_a.txt").exists()); // 원본 삭제됨
        assert!(dest.path().join("file_a.txt").exists()); // 이동됨
    }

    #[test]
    fn test_crud_on_parent_blocked() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 0; // ".."

        app.handle_key(KeyAction::Delete);
        // ".."는 target_count가 0이므로 에러 메시지
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.error_message.is_some());
    }

    #[test]
    fn test_crud_with_selection() {
        let (mut app, _dir) = create_test_app();
        let dest = tempfile::tempdir().unwrap();

        // 두 파일 선택
        app.cursor = 3; // file_a.txt
        app.handle_key(KeyAction::Select);
        app.cursor = 4; // file_b.txt
        app.handle_key(KeyAction::Select);

        app.handle_key(KeyAction::Copy);
        if let AppMode::Input { buffer, prompt, .. } = &mut app.mode {
            assert!(prompt.contains("2개"));
            buffer.clear();
            buffer.push_str(&dest.path().to_string_lossy());
        }
        app.handle_key(KeyAction::InputConfirm);

        assert!(dest.path().join("file_a.txt").exists());
        assert!(dest.path().join("file_b.txt").exists());
    }

    // --- Phase 3 테스트 ---

    #[cfg(unix)]
    #[test]
    fn test_disk_usage() {
        let (app, _dir) = create_test_app();
        let usage = app.disk_usage();
        assert!(usage.is_some());
        let (used, total, percent) = usage.unwrap();
        assert!(total > 0);
        assert!(used <= total);
        assert!(percent <= 100);
    }
}
