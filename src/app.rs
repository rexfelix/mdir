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
    NewFile,
    FileSearch,
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
    Viewer,
    ViewerSearch {
        buffer: String,
        cursor_pos: usize,
    },
    Help {
        scroll: usize,
    },
    Editor,
    EditorConfirmClose,
}

#[derive(Debug, Clone)]
pub struct ViewerState {
    pub filename: String,
    pub lines: Vec<String>,
    pub scroll: usize,
    pub search_query: Option<String>,
    pub search_matches: Vec<usize>,
    pub current_match: usize,
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub filepath: PathBuf,
    pub filename: String,
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub modified: bool,
    pub message: Option<String>,
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
    pub viewer: Option<ViewerState>,
    pub editor: Option<EditorState>,
    pub search_results: bool,
    search_original_dir: Option<PathBuf>,
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
            viewer: None,
            editor: None,
            search_results: false,
            search_original_dir: None,
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
            AppMode::Viewer => crate::event::InputMode::Viewer,
            AppMode::ViewerSearch { .. } => crate::event::InputMode::ViewerSearch,
            AppMode::Editor => crate::event::InputMode::Editor,
            AppMode::EditorConfirmClose => crate::event::InputMode::EditorConfirmClose,
            AppMode::Help { .. } => crate::event::InputMode::Help,
        }
    }

    pub fn handle_key(&mut self, action: KeyAction) {
        match &self.mode {
            AppMode::Normal => self.handle_normal_key(action),
            AppMode::Input { .. } => self.handle_input_key(action),
            AppMode::Confirm { .. } => self.handle_confirm_key(action),
            AppMode::Viewer => self.handle_viewer_key(action),
            AppMode::ViewerSearch { .. } => self.handle_viewer_search_key(action),
            AppMode::Editor => self.handle_editor_key(action),
            AppMode::EditorConfirmClose => self.handle_editor_confirm_key(action),
            AppMode::Help { .. } => self.handle_help_key(action),
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
            KeyAction::Backspace => {
                if self.search_results {
                    self.exit_search_results();
                } else {
                    self.go_parent();
                }
            }
            KeyAction::ToggleHidden => self.toggle_hidden(),
            KeyAction::Select => self.toggle_select(),
            KeyAction::Copy => self.start_copy(),
            KeyAction::Move => self.start_move(),
            KeyAction::Delete => self.start_delete(),
            KeyAction::Rename => self.start_rename(),
            KeyAction::Mkdir => self.start_mkdir(),
            KeyAction::NewFile => self.start_new_file(),
            KeyAction::Edit => self.open_editor(),
            KeyAction::View => self.open_viewer(),
            KeyAction::FileSearch => self.start_file_search(),
            KeyAction::Help => self.open_help(),
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
                    let byte_pos = char_to_byte_pos(buffer, *cursor_pos);
                    buffer.insert(byte_pos, c);
                    *cursor_pos += 1;
                }
            }
            KeyAction::InputBackspace => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    if *cursor_pos > 0 {
                        let prev_byte = char_to_byte_pos(buffer, *cursor_pos - 1);
                        let curr_byte = char_to_byte_pos(buffer, *cursor_pos);
                        buffer.drain(prev_byte..curr_byte);
                        *cursor_pos -= 1;
                    }
                }
            }
            KeyAction::InputDelete => {
                if let AppMode::Input { buffer, cursor_pos, .. } = &mut self.mode {
                    let char_count = buffer.chars().count();
                    if *cursor_pos < char_count {
                        let curr_byte = char_to_byte_pos(buffer, *cursor_pos);
                        let next_byte = char_to_byte_pos(buffer, *cursor_pos + 1);
                        buffer.drain(curr_byte..next_byte);
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
                    if *cursor_pos < buffer.chars().count() {
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
                    *cursor_pos = buffer.chars().count();
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
        // 검색 결과 모드에서 Enter → 해당 파일 위치로 이동
        if self.search_results {
            self.enter_search_result();
            return;
        }
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
        let len = default.chars().count();
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
        let len = default.chars().count();
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
            let len = entry.name.chars().count();
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

    fn start_new_file(&mut self) {
        self.mode = AppMode::Input {
            purpose: InputPurpose::NewFile,
            buffer: String::new(),
            prompt: "새 파일명:".to_string(),
            cursor_pos: 0,
        };
    }

    // --- CRUD 실행 ---

    fn execute_input(&mut self) {
        let mode = self.mode.clone();
        if let AppMode::Input { purpose, buffer, .. } = mode {
            if purpose == InputPurpose::FileSearch {
                self.mode = AppMode::Normal;
                if let Err(e) = self.exec_file_search(&buffer) {
                    self.error_message = Some(e);
                }
                return;
            }
            let result = match purpose {
                InputPurpose::Copy => self.exec_copy(&buffer),
                InputPurpose::Move => self.exec_move(&buffer),
                InputPurpose::Rename => self.exec_rename(&buffer),
                InputPurpose::Mkdir => self.exec_mkdir(&buffer),
                InputPurpose::NewFile => self.exec_new_file(&buffer),
                InputPurpose::FileSearch => unreachable!(),
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

    fn exec_new_file(&self, name: &str) -> Result<(), String> {
        file_ops::create_file(&self.current_dir, name)
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

    // --- 뷰어 ---

    fn open_viewer(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.is_dir() || entry.is_parent {
                return;
            }

            // 10MB 제한
            if entry.size > 10 * 1024 * 1024 {
                self.error_message = Some("파일이 너무 큽니다 (10MB 초과)".to_string());
                return;
            }

            match std::fs::read(&entry.path) {
                Ok(bytes) => {
                    // 바이너리 감지: 첫 512바이트에서 \x00 확인
                    let check_len = bytes.len().min(512);
                    if bytes[..check_len].contains(&0) {
                        self.error_message =
                            Some("바이너리 파일은 열 수 없습니다".to_string());
                        return;
                    }

                    let content = String::from_utf8_lossy(&bytes);
                    let lines: Vec<String> =
                        content.lines().map(|l| l.to_string()).collect();

                    self.viewer = Some(ViewerState {
                        filename: entry.name.clone(),
                        lines,
                        scroll: 0,
                        search_query: None,
                        search_matches: Vec::new(),
                        current_match: 0,
                    });
                    self.mode = AppMode::Viewer;
                }
                Err(e) => {
                    self.error_message =
                        Some(format!("파일 읽기 실패: {}", e));
                }
            }
        }
    }

    fn close_viewer(&mut self) {
        self.viewer = None;
        self.mode = AppMode::Normal;
    }

    fn viewer_visible_lines(&self) -> usize {
        self.terminal_height.saturating_sub(3) as usize
    }

    fn handle_viewer_key(&mut self, action: KeyAction) {
        match action {
            KeyAction::ViewerUp => {
                if let Some(v) = &mut self.viewer {
                    v.scroll = v.scroll.saturating_sub(1);
                }
            }
            KeyAction::ViewerDown => {
                let visible = self.viewer_visible_lines();
                if let Some(v) = &mut self.viewer {
                    if v.scroll + visible < v.lines.len() {
                        v.scroll += 1;
                    }
                }
            }
            KeyAction::ViewerPageUp => {
                let visible = self.viewer_visible_lines();
                if let Some(v) = &mut self.viewer {
                    v.scroll = v.scroll.saturating_sub(visible);
                }
            }
            KeyAction::ViewerPageDown => {
                let visible = self.viewer_visible_lines();
                if let Some(v) = &mut self.viewer {
                    let max_scroll = v.lines.len().saturating_sub(visible);
                    v.scroll = (v.scroll + visible).min(max_scroll);
                }
            }
            KeyAction::ViewerHome => {
                if let Some(v) = &mut self.viewer {
                    v.scroll = 0;
                }
            }
            KeyAction::ViewerEnd => {
                let visible = self.viewer_visible_lines();
                if let Some(v) = &mut self.viewer {
                    v.scroll = v.lines.len().saturating_sub(visible);
                }
            }
            KeyAction::ViewerSearch => {
                self.mode = AppMode::ViewerSearch {
                    buffer: String::new(),
                    cursor_pos: 0,
                };
            }
            KeyAction::ViewerNextMatch => {
                if let Some(v) = &mut self.viewer {
                    if !v.search_matches.is_empty() {
                        v.current_match =
                            (v.current_match + 1) % v.search_matches.len();
                        v.scroll = v.search_matches[v.current_match];
                    }
                }
            }
            KeyAction::ViewerPrevMatch => {
                if let Some(v) = &mut self.viewer {
                    if !v.search_matches.is_empty() {
                        if v.current_match == 0 {
                            v.current_match = v.search_matches.len() - 1;
                        } else {
                            v.current_match -= 1;
                        }
                        v.scroll = v.search_matches[v.current_match];
                    }
                }
            }
            KeyAction::ViewerClose => self.close_viewer(),
            _ => {}
        }
    }

    fn handle_viewer_search_key(&mut self, action: KeyAction) {
        match action {
            KeyAction::ViewerSearchChar(c) => {
                if let AppMode::ViewerSearch { buffer, cursor_pos } = &mut self.mode {
                    let byte_pos = char_to_byte_pos(buffer, *cursor_pos);
                    buffer.insert(byte_pos, c);
                    *cursor_pos += 1;
                }
            }
            KeyAction::ViewerSearchBackspace => {
                if let AppMode::ViewerSearch { buffer, cursor_pos } = &mut self.mode {
                    if *cursor_pos > 0 {
                        let prev_byte = char_to_byte_pos(buffer, *cursor_pos - 1);
                        let curr_byte = char_to_byte_pos(buffer, *cursor_pos);
                        buffer.drain(prev_byte..curr_byte);
                        *cursor_pos -= 1;
                    }
                }
            }
            KeyAction::ViewerSearchConfirm => {
                let query = if let AppMode::ViewerSearch { buffer, .. } = &self.mode {
                    buffer.clone()
                } else {
                    String::new()
                };
                self.execute_viewer_search(&query);
                self.mode = AppMode::Viewer;
            }
            KeyAction::ViewerSearchCancel => {
                self.mode = AppMode::Viewer;
            }
            _ => {}
        }
    }

    fn execute_viewer_search(&mut self, query: &str) {
        if let Some(v) = &mut self.viewer {
            if query.is_empty() {
                v.search_query = None;
                v.search_matches.clear();
                v.current_match = 0;
                return;
            }

            let lower_query = query.to_lowercase();
            let matches: Vec<usize> = v
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(&lower_query))
                .map(|(i, _)| i)
                .collect();

            v.search_query = Some(query.to_string());
            v.search_matches = matches;
            v.current_match = 0;

            // 첫 매치로 스크롤
            if let Some(&first) = v.search_matches.first() {
                v.scroll = first;
            }
        }
    }

    // --- 파일 검색 ---

    fn start_file_search(&mut self) {
        self.mode = AppMode::Input {
            purpose: InputPurpose::FileSearch,
            buffer: String::new(),
            prompt: "검색 패턴:".to_string(),
            cursor_pos: 0,
        };
    }

    fn exec_file_search(&mut self, pattern: &str) -> Result<(), String> {
        if pattern.is_empty() {
            return Err("검색어를 입력해주세요".to_string());
        }

        let results = search_files_recursive(&self.current_dir, pattern, 1000);

        if results.is_empty() {
            return Err(format!("'{}' 검색 결과가 없습니다", pattern));
        }

        self.search_original_dir = Some(self.current_dir.clone());
        self.search_results = true;
        self.entries = results;
        self.cursor = 0;
        self.selected_indices.clear();
        Ok(())
    }

    pub fn exit_search_results(&mut self) {
        if self.search_results {
            if let Some(orig) = self.search_original_dir.take() {
                self.current_dir = orig;
            }
            self.search_results = false;
            self.selected_indices.clear();
            self.load_directory();
        }
    }

    pub fn enter_search_result(&mut self) {
        if !self.search_results {
            return;
        }
        if let Some(entry) = self.entries.get(self.cursor) {
            if let Some(parent) = entry.path.parent() {
                let target_name = entry.name.clone();
                self.current_dir = parent.to_path_buf();
                self.search_results = false;
                self.search_original_dir = None;
                self.selected_indices.clear();
                self.load_directory();
                // 해당 파일에 커서 위치
                self.cursor = self
                    .entries
                    .iter()
                    .position(|e| e.name == target_name)
                    .unwrap_or(0);
            }
        }
    }

    // --- 도움말 ---

    fn open_help(&mut self) {
        self.mode = AppMode::Help { scroll: 0 };
    }

    fn help_visible_lines(&self) -> usize {
        self.terminal_height.saturating_sub(4) as usize
    }

    fn handle_help_key(&mut self, action: KeyAction) {
        let total = generate_help_lines().len();
        let visible = self.help_visible_lines();
        match action {
            KeyAction::HelpUp => {
                if let AppMode::Help { scroll } = &mut self.mode {
                    *scroll = scroll.saturating_sub(1);
                }
            }
            KeyAction::HelpDown => {
                if let AppMode::Help { scroll } = &mut self.mode {
                    if *scroll + visible < total {
                        *scroll += 1;
                    }
                }
            }
            KeyAction::HelpPageUp => {
                if let AppMode::Help { scroll } = &mut self.mode {
                    *scroll = scroll.saturating_sub(visible);
                }
            }
            KeyAction::HelpPageDown => {
                if let AppMode::Help { scroll } = &mut self.mode {
                    let max_scroll = total.saturating_sub(visible);
                    *scroll = (*scroll + visible).min(max_scroll);
                }
            }
            KeyAction::HelpHome => {
                if let AppMode::Help { scroll } = &mut self.mode {
                    *scroll = 0;
                }
            }
            KeyAction::HelpEnd => {
                if let AppMode::Help { scroll } = &mut self.mode {
                    *scroll = total.saturating_sub(visible);
                }
            }
            KeyAction::HelpClose => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
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

    // --- Phase 5: 에디터 ---

    fn open_editor(&mut self) {
        if let Some(entry) = self.entries.get(self.cursor) {
            if entry.is_dir() || entry.is_parent {
                self.error_message = Some("디렉토리는 편집할 수 없습니다".to_string());
                return;
            }

            if entry.size > 10 * 1024 * 1024 {
                self.error_message = Some("파일이 너무 큽니다 (10MB 초과)".to_string());
                return;
            }

            match std::fs::read(&entry.path) {
                Ok(bytes) => {
                    let check_len = bytes.len().min(512);
                    if bytes[..check_len].contains(&0) {
                        self.error_message =
                            Some("바이너리 파일은 편집할 수 없습니다".to_string());
                        return;
                    }

                    let content = String::from_utf8_lossy(&bytes);
                    let lines: Vec<String> = if content.is_empty() {
                        vec!["".to_string()]
                    } else {
                        content.lines().map(|l| l.to_string()).collect()
                    };

                    self.editor = Some(EditorState {
                        filepath: entry.path.clone(),
                        filename: entry.name.clone(),
                        lines,
                        cursor_row: 0,
                        cursor_col: 0,
                        scroll_row: 0,
                        scroll_col: 0,
                        modified: false,
                        message: None,
                    });
                    self.mode = AppMode::Editor;
                }
                Err(e) => {
                    self.error_message = Some(format!("파일 읽기 실패: {}", e));
                }
            }
        }
    }

    fn editor_visible_rows(&self) -> usize {
        self.terminal_height.saturating_sub(4) as usize
    }

    fn editor_adjust_scroll(&mut self) {
        if let Some(ed) = &mut self.editor {
            let visible_rows = self.terminal_height.saturating_sub(4) as usize;
            let visible_cols = self.terminal_width.saturating_sub(8) as usize;

            if ed.cursor_row < ed.scroll_row {
                ed.scroll_row = ed.cursor_row;
            }
            if visible_rows > 0 && ed.cursor_row >= ed.scroll_row + visible_rows {
                ed.scroll_row = ed.cursor_row - visible_rows + 1;
            }

            let cursor_display_col = if ed.cursor_row < ed.lines.len() {
                let line = &ed.lines[ed.cursor_row];
                let prefix: String = line.chars().take(ed.cursor_col).collect();
                unicode_width::UnicodeWidthStr::width(prefix.as_str())
            } else {
                0
            };

            if cursor_display_col < ed.scroll_col {
                ed.scroll_col = cursor_display_col;
            }
            if visible_cols > 0 && cursor_display_col >= ed.scroll_col + visible_cols {
                ed.scroll_col = cursor_display_col - visible_cols + 1;
            }
        }
    }

    fn handle_editor_key(&mut self, action: KeyAction) {
        if let Some(ed) = &mut self.editor {
            ed.message = None;
        }
        match action {
            KeyAction::EditorChar(c) => self.editor_insert_char(c),
            KeyAction::EditorBackspace => self.editor_backspace(),
            KeyAction::EditorDelete => self.editor_delete(),
            KeyAction::EditorEnter => self.editor_enter(),
            KeyAction::EditorUp => self.editor_move_up(),
            KeyAction::EditorDown => self.editor_move_down(),
            KeyAction::EditorLeft => self.editor_move_left(),
            KeyAction::EditorRight => self.editor_move_right(),
            KeyAction::EditorHome => self.editor_home(),
            KeyAction::EditorEnd => self.editor_end(),
            KeyAction::EditorPageUp => self.editor_page_up(),
            KeyAction::EditorPageDown => self.editor_page_down(),
            KeyAction::EditorSave => self.editor_save(),
            KeyAction::EditorClose => self.editor_close(),
            _ => {}
        }
    }

    fn handle_editor_confirm_key(&mut self, action: KeyAction) {
        match action {
            KeyAction::EditorConfirmYes => {
                self.editor = None;
                self.mode = AppMode::Normal;
                self.load_directory();
            }
            KeyAction::EditorConfirmNo => {
                self.mode = AppMode::Editor;
            }
            _ => {}
        }
    }

    fn editor_insert_char(&mut self, c: char) {
        if let Some(ed) = &mut self.editor {
            if ed.cursor_row < ed.lines.len() {
                let byte_pos = char_to_byte_pos(&ed.lines[ed.cursor_row], ed.cursor_col);
                ed.lines[ed.cursor_row].insert(byte_pos, c);
                ed.cursor_col += 1;
                ed.modified = true;
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_backspace(&mut self) {
        if let Some(ed) = &mut self.editor {
            if ed.cursor_col > 0 {
                let byte_pos = char_to_byte_pos(&ed.lines[ed.cursor_row], ed.cursor_col - 1);
                let ch = ed.lines[ed.cursor_row][byte_pos..].chars().next().unwrap();
                ed.lines[ed.cursor_row].remove(byte_pos);
                let _ = ch;
                ed.cursor_col -= 1;
                ed.modified = true;
            } else if ed.cursor_row > 0 {
                let current_line = ed.lines.remove(ed.cursor_row);
                ed.cursor_row -= 1;
                ed.cursor_col = ed.lines[ed.cursor_row].chars().count();
                ed.lines[ed.cursor_row].push_str(&current_line);
                ed.modified = true;
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_delete(&mut self) {
        if let Some(ed) = &mut self.editor {
            let line_len = ed.lines[ed.cursor_row].chars().count();
            if ed.cursor_col < line_len {
                let byte_pos = char_to_byte_pos(&ed.lines[ed.cursor_row], ed.cursor_col);
                ed.lines[ed.cursor_row].remove(byte_pos);
                ed.modified = true;
            } else if ed.cursor_row + 1 < ed.lines.len() {
                let next_line = ed.lines.remove(ed.cursor_row + 1);
                ed.lines[ed.cursor_row].push_str(&next_line);
                ed.modified = true;
            }
        }
    }

    fn editor_enter(&mut self) {
        if let Some(ed) = &mut self.editor {
            let byte_pos = char_to_byte_pos(&ed.lines[ed.cursor_row], ed.cursor_col);
            let rest = ed.lines[ed.cursor_row][byte_pos..].to_string();
            ed.lines[ed.cursor_row].truncate(byte_pos);
            ed.cursor_row += 1;
            ed.lines.insert(ed.cursor_row, rest);
            ed.cursor_col = 0;
            ed.modified = true;
        }
        self.editor_adjust_scroll();
    }

    fn editor_move_up(&mut self) {
        if let Some(ed) = &mut self.editor {
            if ed.cursor_row > 0 {
                ed.cursor_row -= 1;
                let line_len = ed.lines[ed.cursor_row].chars().count();
                if ed.cursor_col > line_len {
                    ed.cursor_col = line_len;
                }
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_move_down(&mut self) {
        if let Some(ed) = &mut self.editor {
            if ed.cursor_row + 1 < ed.lines.len() {
                ed.cursor_row += 1;
                let line_len = ed.lines[ed.cursor_row].chars().count();
                if ed.cursor_col > line_len {
                    ed.cursor_col = line_len;
                }
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_move_left(&mut self) {
        if let Some(ed) = &mut self.editor {
            if ed.cursor_col > 0 {
                ed.cursor_col -= 1;
            } else if ed.cursor_row > 0 {
                ed.cursor_row -= 1;
                ed.cursor_col = ed.lines[ed.cursor_row].chars().count();
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_move_right(&mut self) {
        if let Some(ed) = &mut self.editor {
            let line_len = ed.lines[ed.cursor_row].chars().count();
            if ed.cursor_col < line_len {
                ed.cursor_col += 1;
            } else if ed.cursor_row + 1 < ed.lines.len() {
                ed.cursor_row += 1;
                ed.cursor_col = 0;
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_home(&mut self) {
        if let Some(ed) = &mut self.editor {
            ed.cursor_col = 0;
        }
        self.editor_adjust_scroll();
    }

    fn editor_end(&mut self) {
        if let Some(ed) = &mut self.editor {
            ed.cursor_col = ed.lines[ed.cursor_row].chars().count();
        }
        self.editor_adjust_scroll();
    }

    fn editor_page_up(&mut self) {
        let visible = self.editor_visible_rows();
        if let Some(ed) = &mut self.editor {
            ed.cursor_row = ed.cursor_row.saturating_sub(visible);
            let line_len = ed.lines[ed.cursor_row].chars().count();
            if ed.cursor_col > line_len {
                ed.cursor_col = line_len;
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_page_down(&mut self) {
        let visible = self.editor_visible_rows();
        if let Some(ed) = &mut self.editor {
            ed.cursor_row = (ed.cursor_row + visible).min(ed.lines.len().saturating_sub(1));
            let line_len = ed.lines[ed.cursor_row].chars().count();
            if ed.cursor_col > line_len {
                ed.cursor_col = line_len;
            }
        }
        self.editor_adjust_scroll();
    }

    fn editor_save(&mut self) {
        if let Some(ed) = &mut self.editor {
            match file_ops::save_file(&ed.filepath, &ed.lines) {
                Ok(()) => {
                    ed.modified = false;
                    ed.message = Some("저장 완료".to_string());
                }
                Err(e) => {
                    ed.message = Some(e);
                }
            }
        }
    }

    fn editor_close(&mut self) {
        if let Some(ed) = &self.editor {
            if ed.modified {
                self.mode = AppMode::EditorConfirmClose;
            } else {
                self.editor = None;
                self.mode = AppMode::Normal;
                self.load_directory();
            }
        }
    }
}

pub fn generate_help_lines() -> Vec<String> {
    vec![
        "".to_string(),
        "  ╔══════════════════════════════════════════════════════════════╗".to_string(),
        "  ║              mdir 도움말  (MS-DOS Mdir 3.x 클론)           ║".to_string(),
        "  ╚══════════════════════════════════════════════════════════════╝".to_string(),
        "".to_string(),
        "".to_string(),
        "  [네비게이션]".to_string(),
        "".to_string(),
        "    ↑ / ↓              커서를 위/아래로 이동".to_string(),
        "    ← / →              컬럼 간 이동 (멀티 컬럼 모드에서)".to_string(),
        "    Enter              디렉토리 진입".to_string(),
        "    Backspace          상위 디렉토리로 이동".to_string(),
        "    Home / End         목록의 처음/끝으로 이동".to_string(),
        "    PageUp / PageDown  페이지 단위로 이동".to_string(),
        "".to_string(),
        "".to_string(),
        "  [파일 선택 및 CRUD]".to_string(),
        "".to_string(),
        "    Space              파일/디렉토리 선택 토글 (복수 선택 가능)".to_string(),
        "    C                  선택한 항목을 다른 경로로 복사".to_string(),
        "                       (선택 없으면 커서 위치 항목)".to_string(),
        "    M                  선택한 항목을 다른 경로로 이동".to_string(),
        "                       (선택 없으면 커서 위치 항목)".to_string(),
        "    D                  선택한 항목 삭제 (Y/N 확인 후 실행)".to_string(),
        "                       (선택 없으면 커서 위치 항목)".to_string(),
        "    R                  커서 위치 파일/디렉토리 이름 변경".to_string(),
        "    K                  현재 디렉토리에 새 폴더 생성".to_string(),
        "    N                  현재 디렉토리에 새 빈 파일 생성".to_string(),
        "".to_string(),
        "".to_string(),
        "  [내부 뷰어]  (V 키로 진입)".to_string(),
        "".to_string(),
        "    V                  텍스트 파일 내용 보기 (10MB 이하만 가능)".to_string(),
        "                       바이너리 파일은 자동 감지되어 열리지 않음".to_string(),
        "".to_string(),
        "    뷰어 모드 단축키:".to_string(),
        "      ↑ / ↓            한 줄 스크롤".to_string(),
        "      PageUp / PageDown  페이지 단위 스크롤".to_string(),
        "      Home / End       파일 처음/끝으로 이동".to_string(),
        "      /                텍스트 검색 (검색어 입력 후 Enter)".to_string(),
        "      n                다음 검색 매치로 이동".to_string(),
        "      N                이전 검색 매치로 이동".to_string(),
        "      Q / Esc          뷰어 닫기".to_string(),
        "".to_string(),
        "    검색 매치는 노란색으로, 현재 매치는 노란색 반전으로 강조".to_string(),
        "".to_string(),
        "".to_string(),
        "  [내부 에디터]  (E 키로 진입)".to_string(),
        "".to_string(),
        "    E                  텍스트 파일 편집 (10MB 이하, 바이너리 제외)".to_string(),
        "".to_string(),
        "    에디터 모드 단축키:".to_string(),
        "      ↑ / ↓            커서 위/아래 이동".to_string(),
        "      ← / →            커서 좌/우 이동 (줄 경계 시 줄 이동)".to_string(),
        "      Home / End       줄의 처음/끝으로 이동".to_string(),
        "      PageUp / PageDown  페이지 단위 이동".to_string(),
        "      Backspace        커서 앞 문자 삭제 (줄 시작이면 윗줄과 병합)".to_string(),
        "      Delete           커서 위치 문자 삭제 (줄 끝이면 아랫줄과 병합)".to_string(),
        "      Enter            현재 위치에서 줄 분할".to_string(),
        "      Ctrl+S           파일 저장".to_string(),
        "      Esc              에디터 닫기 (수정 시 저장 확인)".to_string(),
        "".to_string(),
        "    * 수정된 파일은 타이틀에 [*] 표시".to_string(),
        "    * 한글 등 멀티바이트 문자 편집 지원".to_string(),
        "".to_string(),
        "".to_string(),
        "  [파일 검색]  (F 키로 진입)".to_string(),
        "".to_string(),
        "    F                  현재 디렉토리 하위 재귀 검색".to_string(),
        "                       패턴을 입력하면 하위 모든 파일을 탐색".to_string(),
        "".to_string(),
        "    패턴 예시:".to_string(),
        "      *.rs             모든 Rust 소스 파일".to_string(),
        "      test*            \"test\"로 시작하는 파일".to_string(),
        "      *.tar.gz         모든 tar.gz 압축 파일".to_string(),
        "      README?          README + 임의 한 글자".to_string(),
        "      config           \"config\" 포함 파일 (부분 일치)".to_string(),
        "".to_string(),
        "    검색 결과에서:".to_string(),
        "      Enter            해당 파일이 있는 디렉토리로 이동".to_string(),
        "      Backspace        원래 위치로 복귀".to_string(),
        "".to_string(),
        "    * 패턴은 대소문자를 구분하지 않음".to_string(),
        "    * 와일드카드 없이 입력하면 부분 일치로 검색".to_string(),
        "".to_string(),
        "".to_string(),
        "  [입력 모드]  (이름변경, 폴더/파일 생성, 복사/이동 경로 입력 시)".to_string(),
        "".to_string(),
        "    ← / →              커서 좌/우 이동".to_string(),
        "    Home / End         입력 시작/끝으로 이동".to_string(),
        "    Delete             커서 위치 문자 삭제".to_string(),
        "    Backspace          커서 앞 문자 삭제".to_string(),
        "    Enter              입력 확인".to_string(),
        "    Esc                입력 취소".to_string(),
        "".to_string(),
        "".to_string(),
        "  [표시 설정]".to_string(),
        "".to_string(),
        "    H                  숨김 파일(. 시작) 표시/숨김 토글".to_string(),
        "".to_string(),
        "".to_string(),
        "  [컬럼 레이아웃]".to_string(),
        "".to_string(),
        "    터미널 너비  80 미만    →  1컬럼".to_string(),
        "    터미널 너비  80~119     →  2컬럼".to_string(),
        "    터미널 너비 120 이상    →  3컬럼".to_string(),
        "".to_string(),
        "".to_string(),
        "  [파일 타입별 색상]".to_string(),
        "".to_string(),
        "    디렉토리             하늘색 (굵게)".to_string(),
        "    심볼릭 링크          보라색".to_string(),
        "    압축 파일            노란색  (.tar.gz, .zip, .rar, .7z 등)".to_string(),
        "    실행 파일            녹색".to_string(),
        "    일반 파일            흰색".to_string(),
        "".to_string(),
        "    * 스타일 우선순위: 커서(흰 배경) > 선택(노란색) > 타입별 색상".to_string(),
        "".to_string(),
        "".to_string(),
        "  [기타]".to_string(),
        "".to_string(),
        "    ?                  이 도움말 표시".to_string(),
        "    Q / F10            프로그램 종료".to_string(),
        "    Ctrl+C             프로그램 종료".to_string(),
        "".to_string(),
        "".to_string(),
        "  Q, Esc, ? 키를 누르면 도움말을 닫습니다.".to_string(),
        "".to_string(),
    ]
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

/// 재귀적으로 파일을 검색한다. 패턴은 대소문자 무시 + 간단한 glob (* 와일드카드).
fn search_files_recursive(
    dir: &std::path::Path,
    pattern: &str,
    max_results: usize,
) -> Vec<FileEntry> {
    let mut results = Vec::new();
    search_files_inner(dir, pattern, max_results, &mut results);
    file_entry::sort_entries(&mut results);
    results
}

fn search_files_inner(
    dir: &std::path::Path,
    pattern: &str,
    max_results: usize,
    results: &mut Vec<FileEntry>,
) {
    if results.len() >= max_results {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if results.len() >= max_results {
            return;
        }
        let path = entry.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // 숨김 파일 건너뜀
        if name.starts_with('.') {
            continue;
        }

        if glob_match(pattern, &name) {
            if let Ok(fe) = FileEntry::from_path(&path) {
                results.push(fe);
            }
        }

        if path.is_dir() {
            search_files_inner(&path, pattern, max_results, results);
        }
    }
}

/// char 인덱스를 byte 인덱스로 변환한다.
/// cursor_pos 등 char 단위 위치를 String의 byte 위치로 변환할 때 사용.
fn char_to_byte_pos(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// 간단한 glob 매칭: * = 임의 문자열, ? = 임의 한 문자, 대소문자 무시.
/// 패턴에 와일드카드가 없으면 contains 매칭.
fn glob_match(pattern: &str, name: &str) -> bool {
    let lower_pattern = pattern.to_lowercase();
    let lower_name = name.to_lowercase();

    if !lower_pattern.contains('*') && !lower_pattern.contains('?') {
        return lower_name.contains(&lower_pattern);
    }

    glob_match_inner(lower_pattern.as_bytes(), lower_name.as_bytes())
}

fn glob_match_inner(pattern: &[u8], name: &[u8]) -> bool {
    let mut pi = 0;
    let mut ni = 0;
    let mut star_pi = usize::MAX;
    let mut star_ni = 0;

    while ni < name.len() {
        if pi < pattern.len() && pattern[pi] == b'?' {
            pi += 1;
            ni += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = pi;
            star_ni = ni;
            pi += 1;
        } else if pi < pattern.len() && pattern[pi] == name[ni] {
            pi += 1;
            ni += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ni += 1;
            ni = star_ni;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
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
    fn test_start_new_file_enters_input_mode() {
        let (mut app, _) = create_test_app();
        app.handle_key(KeyAction::NewFile);
        match &app.mode {
            AppMode::Input { purpose, buffer, cursor_pos, .. } => {
                assert_eq!(*purpose, InputPurpose::NewFile);
                assert!(buffer.is_empty());
                assert_eq!(*cursor_pos, 0);
            }
            _ => panic!("NewFile 키로 Input 모드 진입 실패"),
        }
    }

    #[test]
    fn test_new_file_execute() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::NewFile);
        for c in "test_new.txt".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }
        app.handle_key(KeyAction::InputConfirm);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.current_dir.join("test_new.txt").is_file());
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
            assert_eq!(*cursor_pos, buffer.chars().count());
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

    // --- Phase 4 테스트 ---

    #[test]
    fn test_viewer_open_text_file() {
        let (mut app, _dir) = create_test_app();
        // file_a.txt 에 커서 (index 3)
        app.cursor = 3;
        app.handle_key(KeyAction::View);
        assert_eq!(app.mode, AppMode::Viewer);
        assert!(app.viewer.is_some());
        let v = app.viewer.as_ref().unwrap();
        assert_eq!(v.filename, "file_a.txt");
        assert_eq!(v.lines.len(), 1); // "aaa"
        assert_eq!(v.scroll, 0);
    }

    #[test]
    fn test_viewer_open_directory_ignored() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 1; // alpha_dir
        app.handle_key(KeyAction::View);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.viewer.is_none());
    }

    #[test]
    fn test_viewer_close() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 3;
        app.handle_key(KeyAction::View);
        assert_eq!(app.mode, AppMode::Viewer);

        app.handle_key(KeyAction::ViewerClose);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.viewer.is_none());
    }

    #[test]
    fn test_viewer_scroll() {
        let (mut app, dir) = create_test_app();
        // 여러 줄 파일 생성
        let content: String = (0..100).map(|i| format!("line {}\n", i)).collect();
        fs::write(dir.path().join("long.txt"), &content).unwrap();
        app.load_directory();

        // long.txt 찾기
        let idx = app.entries.iter().position(|e| e.name == "long.txt").unwrap();
        app.cursor = idx;
        app.update_layout(80, 24); // visible = 24 - 3 = 21 lines
        app.handle_key(KeyAction::View);

        let v = app.viewer.as_ref().unwrap();
        assert_eq!(v.scroll, 0);
        assert_eq!(v.lines.len(), 100);

        // 아래로 스크롤
        app.handle_key(KeyAction::ViewerDown);
        assert_eq!(app.viewer.as_ref().unwrap().scroll, 1);

        // 위로 스크롤
        app.handle_key(KeyAction::ViewerUp);
        assert_eq!(app.viewer.as_ref().unwrap().scroll, 0);

        // 위 경계: 0에서 더 올라가지 않음
        app.handle_key(KeyAction::ViewerUp);
        assert_eq!(app.viewer.as_ref().unwrap().scroll, 0);

        // End로 끝으로
        app.handle_key(KeyAction::ViewerEnd);
        let v = app.viewer.as_ref().unwrap();
        assert!(v.scroll > 0);
        assert!(v.scroll + 21 >= v.lines.len());

        // Home으로 처음으로
        app.handle_key(KeyAction::ViewerHome);
        assert_eq!(app.viewer.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn test_viewer_search() {
        let (mut app, dir) = create_test_app();
        let content = "apple\nbanana\ncherry\napricot\nblueberry\n";
        fs::write(dir.path().join("fruits.txt"), content).unwrap();
        app.load_directory();

        let idx = app.entries.iter().position(|e| e.name == "fruits.txt").unwrap();
        app.cursor = idx;
        app.update_layout(80, 24);
        app.handle_key(KeyAction::View);

        // / 키로 검색 모드 진입
        app.handle_key(KeyAction::ViewerSearch);
        assert!(matches!(app.mode, AppMode::ViewerSearch { .. }));

        // "ap" 입력
        app.handle_key(KeyAction::ViewerSearchChar('a'));
        app.handle_key(KeyAction::ViewerSearchChar('p'));
        app.handle_key(KeyAction::ViewerSearchConfirm);

        assert_eq!(app.mode, AppMode::Viewer);
        let v = app.viewer.as_ref().unwrap();
        assert_eq!(v.search_query, Some("ap".to_string()));
        assert_eq!(v.search_matches.len(), 2); // apple, apricot
        assert_eq!(v.search_matches[0], 0);    // line 0: apple
        assert_eq!(v.search_matches[1], 3);    // line 3: apricot

        // n으로 다음 매치
        app.handle_key(KeyAction::ViewerNextMatch);
        let v = app.viewer.as_ref().unwrap();
        assert_eq!(v.current_match, 1);
        assert_eq!(v.scroll, 3);

        // N으로 이전 매치
        app.handle_key(KeyAction::ViewerPrevMatch);
        let v = app.viewer.as_ref().unwrap();
        assert_eq!(v.current_match, 0);
        assert_eq!(v.scroll, 0);
    }

    #[test]
    fn test_viewer_search_cancel() {
        let (mut app, _dir) = create_test_app();
        app.cursor = 3;
        app.handle_key(KeyAction::View);
        app.handle_key(KeyAction::ViewerSearch);
        assert!(matches!(app.mode, AppMode::ViewerSearch { .. }));

        app.handle_key(KeyAction::ViewerSearchCancel);
        assert_eq!(app.mode, AppMode::Viewer);
    }

    #[test]
    fn test_viewer_binary_file_rejected() {
        let (mut app, dir) = create_test_app();
        // 바이너리 파일 생성 (null 바이트 포함)
        let mut binary_data = vec![0x89, 0x50, 0x4E, 0x47, 0x00, 0x00];
        binary_data.extend_from_slice(&[0x42; 100]);
        fs::write(dir.path().join("image.png"), &binary_data).unwrap();
        app.load_directory();

        let idx = app.entries.iter().position(|e| e.name == "image.png").unwrap();
        app.cursor = idx;
        app.handle_key(KeyAction::View);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.viewer.is_none());
        assert!(app.error_message.is_some());
        assert!(app.error_message.as_ref().unwrap().contains("바이너리"));
    }

    #[test]
    fn test_file_search() {
        let (mut app, dir) = create_test_app();
        // 하위 디렉토리에 파일 생성
        fs::write(dir.path().join("alpha_dir").join("nested.txt"), "nested").unwrap();

        app.handle_key(KeyAction::FileSearch);
        match &app.mode {
            AppMode::Input { purpose, .. } => {
                assert_eq!(*purpose, InputPurpose::FileSearch);
            }
            _ => panic!("FileSearch 키로 Input 모드 진입 실패"),
        }

        // "txt" 검색
        for c in "txt".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }
        app.handle_key(KeyAction::InputConfirm);

        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.search_results);
        // file_a.txt, file_b.txt, nested.txt
        assert!(app.entries.len() >= 3);
        assert!(app.entries.iter().any(|e| e.name == "nested.txt"));
    }

    #[test]
    fn test_file_search_exit() {
        let (mut app, _dir) = create_test_app();
        let original_dir = app.current_dir.clone();

        app.handle_key(KeyAction::FileSearch);
        for c in "file".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }
        app.handle_key(KeyAction::InputConfirm);
        assert!(app.search_results);

        // Backspace로 검색 결과 나가기
        app.handle_key(KeyAction::Backspace);
        assert!(!app.search_results);
        assert_eq!(app.current_dir, original_dir);
    }

    #[test]
    fn test_file_search_enter_result() {
        let (mut app, dir) = create_test_app();
        fs::write(dir.path().join("alpha_dir").join("target.txt"), "found").unwrap();

        app.handle_key(KeyAction::FileSearch);
        for c in "target".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }
        app.handle_key(KeyAction::InputConfirm);
        assert!(app.search_results);

        // Enter로 결과 위치로 이동
        app.cursor = 0;
        app.handle_key(KeyAction::Enter);
        assert!(!app.search_results);
        assert!(app.current_dir.ends_with("alpha_dir"));
        assert_eq!(app.entries[app.cursor].name, "target.txt");
    }

    #[test]
    fn test_file_search_no_results() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::FileSearch);
        for c in "nonexistent_xyz".chars() {
            app.handle_key(KeyAction::InputChar(c));
        }
        app.handle_key(KeyAction::InputConfirm);
        assert!(!app.search_results);
        assert!(app.error_message.is_some());
        assert!(app.error_message.as_ref().unwrap().contains("검색 결과가 없습니다"));
    }

    #[test]
    fn test_glob_match() {
        assert!(super::glob_match("*.txt", "readme.txt"));
        assert!(super::glob_match("*.txt", "README.TXT"));
        assert!(!super::glob_match("*.txt", "readme.rs"));
        assert!(super::glob_match("test?", "test1"));
        assert!(super::glob_match("test?", "testA"));
        assert!(!super::glob_match("test?", "test12"));
        assert!(super::glob_match("*", "anything"));
        assert!(super::glob_match("file", "file_a.txt")); // contains match
        assert!(super::glob_match("FILE", "file_a.txt")); // case insensitive
    }

    #[test]
    fn test_char_to_byte_pos() {
        assert_eq!(super::char_to_byte_pos("hello", 0), 0);
        assert_eq!(super::char_to_byte_pos("hello", 3), 3);
        assert_eq!(super::char_to_byte_pos("hello", 5), 5);
        // 한글: 각 문자 3바이트
        assert_eq!(super::char_to_byte_pos("한글테스트", 0), 0);
        assert_eq!(super::char_to_byte_pos("한글테스트", 1), 3);
        assert_eq!(super::char_to_byte_pos("한글테스트", 2), 6);
        assert_eq!(super::char_to_byte_pos("한글테스트", 5), 15);
        // 혼합
        assert_eq!(super::char_to_byte_pos("a한b", 0), 0);
        assert_eq!(super::char_to_byte_pos("a한b", 1), 1);
        assert_eq!(super::char_to_byte_pos("a한b", 2), 4);
        assert_eq!(super::char_to_byte_pos("a한b", 3), 5);
        // 범위 초과 시 len 반환
        assert_eq!(super::char_to_byte_pos("abc", 10), 3);
    }

    #[test]
    fn test_multibyte_input_cursor() {
        let (mut app, _dir) = create_test_app();
        app.handle_key(KeyAction::Mkdir);

        // 한글 입력: "가나다"
        app.handle_key(KeyAction::InputChar('가'));
        app.handle_key(KeyAction::InputChar('나'));
        app.handle_key(KeyAction::InputChar('다'));

        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "가나다");
            assert_eq!(*cursor_pos, 3); // char 단위
        }

        // Left 2번 → cursor_pos = 1 ("가|나다")
        app.handle_key(KeyAction::InputCursorLeft);
        app.handle_key(KeyAction::InputCursorLeft);
        if let AppMode::Input { cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, 1);
        }

        // 중간에 'X' 삽입 → "가X나다"
        app.handle_key(KeyAction::InputChar('X'));
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "가X나다");
            assert_eq!(*cursor_pos, 2);
        }

        // Backspace → "가나다"
        app.handle_key(KeyAction::InputBackspace);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "가나다");
            assert_eq!(*cursor_pos, 1);
        }

        // Delete → "가다"
        app.handle_key(KeyAction::InputDelete);
        if let AppMode::Input { buffer, cursor_pos, .. } = &app.mode {
            assert_eq!(buffer, "가다");
            assert_eq!(*cursor_pos, 1);
        }

        // End → cursor_pos = 2
        app.handle_key(KeyAction::InputCursorEnd);
        if let AppMode::Input { cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, 2);
        }

        // Home → cursor_pos = 0
        app.handle_key(KeyAction::InputCursorHome);
        if let AppMode::Input { cursor_pos, .. } = &app.mode {
            assert_eq!(*cursor_pos, 0);
        }
    }

    // --- 도움말 테스트 ---

    #[test]
    fn test_open_help_enters_help_mode() {
        let (mut app, _) = create_test_app();
        app.handle_key(KeyAction::Help);
        match &app.mode {
            AppMode::Help { scroll } => {
                assert_eq!(*scroll, 0);
            }
            _ => panic!("Help 키로 Help 모드 진입 실패"),
        }
    }

    #[test]
    fn test_help_close_returns_normal() {
        let (mut app, _) = create_test_app();
        app.handle_key(KeyAction::Help);
        assert!(matches!(app.mode, AppMode::Help { .. }));
        app.handle_key(KeyAction::HelpClose);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_help_scroll() {
        let (mut app, _) = create_test_app();
        app.terminal_height = 20;
        app.handle_key(KeyAction::Help);

        // 아래로 스크롤
        app.handle_key(KeyAction::HelpDown);
        if let AppMode::Help { scroll } = &app.mode {
            assert_eq!(*scroll, 1);
        } else {
            panic!("Help 모드가 아님");
        }

        // 위로 스크롤
        app.handle_key(KeyAction::HelpUp);
        if let AppMode::Help { scroll } = &app.mode {
            assert_eq!(*scroll, 0);
        }

        // Home: 처음으로
        app.handle_key(KeyAction::HelpDown);
        app.handle_key(KeyAction::HelpDown);
        app.handle_key(KeyAction::HelpHome);
        if let AppMode::Help { scroll } = &app.mode {
            assert_eq!(*scroll, 0);
        }
    }

    #[test]
    fn test_generate_help_lines_not_empty() {
        let lines = generate_help_lines();
        assert!(!lines.is_empty());
        // 주요 섹션이 포함되어야 한다
        let joined = lines.join("\n");
        assert!(joined.contains("네비게이션"));
        assert!(joined.contains("CRUD"));
        assert!(joined.contains("뷰어"));
        assert!(joined.contains("에디터"));
        assert!(joined.contains("검색"));
        assert!(joined.contains("입력 모드"));
    }

    // --- 에디터 테스트 (Phase 5) ---

    /// 테스트용 에디터를 직접 세팅하는 헬퍼.
    /// 파일 시스템 의존 없이 EditorState를 주입한다.
    fn setup_editor(app: &mut App, lines: Vec<&str>, filepath: PathBuf) {
        app.editor = Some(EditorState {
            filepath,
            filename: "test.txt".to_string(),
            lines: lines.into_iter().map(|s| s.to_string()).collect(),
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            modified: false,
            message: None,
        });
        app.mode = AppMode::Editor;
    }

    #[test]
    fn test_editor_open_text_file() {
        let (mut app, _dir) = create_test_app();
        // file_a.txt 위치 찾기
        let idx = app.entries.iter().position(|e| e.name == "file_a.txt").unwrap();
        app.cursor = idx;
        app.handle_key(KeyAction::Edit);
        assert_eq!(app.mode, AppMode::Editor);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines, vec!["aaa"]);
        assert_eq!(ed.cursor_row, 0);
        assert_eq!(ed.cursor_col, 0);
        assert!(!ed.modified);
    }

    #[test]
    fn test_editor_open_directory_rejected() {
        let (mut app, _dir) = create_test_app();
        let idx = app.entries.iter().position(|e| e.name == "alpha_dir").unwrap();
        app.cursor = idx;
        app.handle_key(KeyAction::Edit);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.error_message.as_ref().unwrap().contains("디렉토리"));
    }

    #[test]
    fn test_editor_open_binary_rejected() {
        let (mut app, dir) = create_test_app();
        // 바이너리 파일 생성
        fs::write(dir.path().join("binary.dat"), &[0u8, 1, 2, 3, 0]).unwrap();
        app.load_directory();
        let idx = app.entries.iter().position(|e| e.name == "binary.dat").unwrap();
        app.cursor = idx;
        app.handle_key(KeyAction::Edit);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.error_message.as_ref().unwrap().contains("바이너리"));
    }

    #[test]
    fn test_editor_insert_char() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorChar('X'));
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines[0], "Xabc");
        assert_eq!(ed.cursor_col, 1);
        assert!(ed.modified);
    }

    #[test]
    fn test_editor_insert_char_middle() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 2;
        app.handle_key(KeyAction::EditorChar('X'));
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines[0], "abXc");
        assert_eq!(ed.cursor_col, 3);
    }

    #[test]
    fn test_editor_insert_multibyte() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["가나다"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 1; // "가|나다"
        app.handle_key(KeyAction::EditorChar('X'));
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines[0], "가X나다");
        assert_eq!(ed.cursor_col, 2);
    }

    #[test]
    fn test_editor_backspace_within_line() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 2;
        app.handle_key(KeyAction::EditorBackspace);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines[0], "ac");
        assert_eq!(ed.cursor_col, 1);
        assert!(ed.modified);
    }

    #[test]
    fn test_editor_backspace_line_merge() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["hello", "world"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_row = 1;
        app.editor.as_mut().unwrap().cursor_col = 0;
        app.handle_key(KeyAction::EditorBackspace);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines.len(), 1);
        assert_eq!(ed.lines[0], "helloworld");
        assert_eq!(ed.cursor_row, 0);
        assert_eq!(ed.cursor_col, 5);
    }

    #[test]
    fn test_editor_backspace_at_beginning_noop() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorBackspace);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines[0], "abc");
        assert!(!ed.modified);
    }

    #[test]
    fn test_editor_delete_within_line() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorDelete);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines[0], "bc");
        assert_eq!(ed.cursor_col, 0);
        assert!(ed.modified);
    }

    #[test]
    fn test_editor_delete_line_merge() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc", "def"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 3; // 줄 끝
        app.handle_key(KeyAction::EditorDelete);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines.len(), 1);
        assert_eq!(ed.lines[0], "abcdef");
    }

    #[test]
    fn test_editor_enter_splits_line() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abcdef"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 3;
        app.handle_key(KeyAction::EditorEnter);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines.len(), 2);
        assert_eq!(ed.lines[0], "abc");
        assert_eq!(ed.lines[1], "def");
        assert_eq!(ed.cursor_row, 1);
        assert_eq!(ed.cursor_col, 0);
        assert!(ed.modified);
    }

    #[test]
    fn test_editor_enter_at_beginning() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorEnter);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines.len(), 2);
        assert_eq!(ed.lines[0], "");
        assert_eq!(ed.lines[1], "abc");
        assert_eq!(ed.cursor_row, 1);
        assert_eq!(ed.cursor_col, 0);
    }

    #[test]
    fn test_editor_enter_at_end() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 3;
        app.handle_key(KeyAction::EditorEnter);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines.len(), 2);
        assert_eq!(ed.lines[0], "abc");
        assert_eq!(ed.lines[1], "");
    }

    #[test]
    fn test_editor_move_up_down() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["aaa", "bbb", "ccc"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorDown);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 1);
        app.handle_key(KeyAction::EditorDown);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 2);
        // 마지막 줄에서 Down: 변화 없음
        app.handle_key(KeyAction::EditorDown);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 2);
        app.handle_key(KeyAction::EditorUp);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 1);
        app.handle_key(KeyAction::EditorUp);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 0);
        // 첫 줄에서 Up: 변화 없음
        app.handle_key(KeyAction::EditorUp);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 0);
    }

    #[test]
    fn test_editor_move_up_clamps_col() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abcdef", "ab"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 5; // 첫 줄에서 col=5
        app.handle_key(KeyAction::EditorDown);
        // 둘째 줄은 2글자뿐이므로 col이 2로 클램프
        assert_eq!(app.editor.as_ref().unwrap().cursor_col, 2);
    }

    #[test]
    fn test_editor_move_left_right() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc", "def"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorRight);
        assert_eq!(app.editor.as_ref().unwrap().cursor_col, 1);
        app.handle_key(KeyAction::EditorRight);
        app.handle_key(KeyAction::EditorRight);
        assert_eq!(app.editor.as_ref().unwrap().cursor_col, 3); // 줄 끝
        // Right at end of line → wrap to next line col 0
        app.handle_key(KeyAction::EditorRight);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.cursor_row, 1);
        assert_eq!(ed.cursor_col, 0);
    }

    #[test]
    fn test_editor_move_left_wraps_prev_line() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc", "def"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_row = 1;
        app.editor.as_mut().unwrap().cursor_col = 0;
        // Left at beginning of line → prev line end
        app.handle_key(KeyAction::EditorLeft);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.cursor_row, 0);
        assert_eq!(ed.cursor_col, 3);
    }

    #[test]
    fn test_editor_home_end() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["hello world"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().cursor_col = 5;
        app.handle_key(KeyAction::EditorHome);
        assert_eq!(app.editor.as_ref().unwrap().cursor_col, 0);
        app.handle_key(KeyAction::EditorEnd);
        assert_eq!(app.editor.as_ref().unwrap().cursor_col, 11);
    }

    #[test]
    fn test_editor_page_up_down() {
        let (mut app, dir) = create_test_app();
        let lines: Vec<&str> = (0..50).map(|_| "line").collect();
        setup_editor(&mut app, lines, dir.path().join("t.txt"));
        app.terminal_height = 24; // visible_rows = 24-4 = 20
        app.handle_key(KeyAction::EditorPageDown);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 20);
        app.handle_key(KeyAction::EditorPageDown);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 40);
        app.handle_key(KeyAction::EditorPageDown);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 49); // clamped
        app.handle_key(KeyAction::EditorPageUp);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 29);
        app.handle_key(KeyAction::EditorPageUp);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 9);
        app.handle_key(KeyAction::EditorPageUp);
        assert_eq!(app.editor.as_ref().unwrap().cursor_row, 0);
    }

    #[test]
    fn test_editor_save() {
        let (mut app, dir) = create_test_app();
        let filepath = dir.path().join("save_test.txt");
        fs::write(&filepath, "original\n").unwrap();
        setup_editor(&mut app, vec!["modified", "content"], filepath.clone());
        app.editor.as_mut().unwrap().modified = true;
        app.handle_key(KeyAction::EditorSave);
        let ed = app.editor.as_ref().unwrap();
        assert!(!ed.modified);
        assert_eq!(ed.message.as_ref().unwrap(), "저장 완료");
        // 파일 내용 확인
        let content = fs::read_to_string(&filepath).unwrap();
        assert_eq!(content, "modified\ncontent\n");
    }

    #[test]
    fn test_editor_close_unmodified() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.handle_key(KeyAction::EditorClose);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.editor.is_none());
    }

    #[test]
    fn test_editor_close_modified_asks_confirm() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().modified = true;
        app.handle_key(KeyAction::EditorClose);
        assert_eq!(app.mode, AppMode::EditorConfirmClose);
        assert!(app.editor.is_some()); // 아직 열려 있음
    }

    #[test]
    fn test_editor_confirm_yes_closes() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().modified = true;
        app.handle_key(KeyAction::EditorClose);
        assert_eq!(app.mode, AppMode::EditorConfirmClose);
        app.handle_key(KeyAction::EditorConfirmYes);
        assert_eq!(app.mode, AppMode::Normal);
        assert!(app.editor.is_none());
    }

    #[test]
    fn test_editor_confirm_no_returns_editor() {
        let (mut app, dir) = create_test_app();
        setup_editor(&mut app, vec!["abc"], dir.path().join("t.txt"));
        app.editor.as_mut().unwrap().modified = true;
        app.handle_key(KeyAction::EditorClose);
        assert_eq!(app.mode, AppMode::EditorConfirmClose);
        app.handle_key(KeyAction::EditorConfirmNo);
        assert_eq!(app.mode, AppMode::Editor);
        assert!(app.editor.is_some());
    }

    #[test]
    fn test_editor_empty_file() {
        let (mut app, dir) = create_test_app();
        let filepath = dir.path().join("empty.txt");
        fs::write(&filepath, "").unwrap();
        app.load_directory();
        let idx = app.entries.iter().position(|e| e.name == "empty.txt").unwrap();
        app.cursor = idx;
        app.handle_key(KeyAction::Edit);
        assert_eq!(app.mode, AppMode::Editor);
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.lines, vec![""]);
    }
}
