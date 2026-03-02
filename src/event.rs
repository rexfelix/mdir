use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub enum AppEvent {
    Key(KeyAction),
    Resize(u16, u16),
    None,
}

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    // 네비게이션
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Backspace,
    // 토글
    ToggleHidden,
    // Phase 2: 선택 및 CRUD
    Select,
    Copy,
    Move,
    Delete,
    Rename,
    Mkdir,
    // 입력 모드 전용
    InputChar(char),
    InputBackspace,
    InputDelete,
    InputConfirm,
    InputCancel,
    InputCursorLeft,
    InputCursorRight,
    InputCursorHome,
    InputCursorEnd,
    // Phase 4: 뷰어 및 검색
    View,
    FileSearch,
    // 확인 모드 전용
    ConfirmYes,
    ConfirmNo,
    // 뷰어 모드 전용
    ViewerUp,
    ViewerDown,
    ViewerPageUp,
    ViewerPageDown,
    ViewerHome,
    ViewerEnd,
    ViewerSearch,
    ViewerNextMatch,
    ViewerPrevMatch,
    ViewerClose,
    // 뷰어 검색 입력 전용
    ViewerSearchChar(char),
    ViewerSearchBackspace,
    ViewerSearchConfirm,
    ViewerSearchCancel,
    // 기타
    Quit,
    Noop,
}

/// 현재 앱 모드에 따라 키 매핑이 달라진다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Input,
    Confirm,
    Viewer,
    ViewerSearch,
}

pub fn poll_event_with_mode(timeout: Duration, mode: InputMode) -> std::io::Result<AppEvent> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => {
                let action = match mode {
                    InputMode::Normal => map_key_normal(key),
                    InputMode::Input => map_key_input(key),
                    InputMode::Confirm => map_key_confirm(key),
                    InputMode::Viewer => map_key_viewer(key),
                    InputMode::ViewerSearch => map_key_viewer_search(key),
                };
                Ok(AppEvent::Key(action))
            }
            Event::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
            _ => Ok(AppEvent::None),
        }
    } else {
        Ok(AppEvent::None)
    }
}

fn map_key_normal(key: KeyEvent) -> KeyAction {
    match key.code {
        KeyCode::Up => KeyAction::MoveUp,
        KeyCode::Down => KeyAction::MoveDown,
        KeyCode::Left => KeyAction::MoveLeft,
        KeyCode::Right => KeyAction::MoveRight,
        KeyCode::Home => KeyAction::Home,
        KeyCode::End => KeyAction::End,
        KeyCode::PageUp => KeyAction::PageUp,
        KeyCode::PageDown => KeyAction::PageDown,
        KeyCode::Enter => KeyAction::Enter,
        KeyCode::Backspace => KeyAction::Backspace,
        KeyCode::Char('q') | KeyCode::Char('Q') => KeyAction::Quit,
        KeyCode::Char('h') | KeyCode::Char('H') => KeyAction::ToggleHidden,
        KeyCode::Char(' ') => KeyAction::Select,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
        KeyCode::Char('c') | KeyCode::Char('C') => KeyAction::Copy,
        KeyCode::Char('m') | KeyCode::Char('M') => KeyAction::Move,
        KeyCode::Char('d') | KeyCode::Char('D') => KeyAction::Delete,
        KeyCode::Char('r') | KeyCode::Char('R') => KeyAction::Rename,
        KeyCode::Char('k') | KeyCode::Char('K') => KeyAction::Mkdir,
        KeyCode::Char('v') | KeyCode::Char('V') => KeyAction::View,
        KeyCode::Char('f') | KeyCode::Char('F') => KeyAction::FileSearch,
        KeyCode::F(10) => KeyAction::Quit,
        _ => KeyAction::Noop,
    }
}

fn map_key_input(key: KeyEvent) -> KeyAction {
    match key.code {
        KeyCode::Enter => KeyAction::InputConfirm,
        KeyCode::Esc => KeyAction::InputCancel,
        KeyCode::Backspace => KeyAction::InputBackspace,
        KeyCode::Delete => KeyAction::InputDelete,
        KeyCode::Left => KeyAction::InputCursorLeft,
        KeyCode::Right => KeyAction::InputCursorRight,
        KeyCode::Home => KeyAction::InputCursorHome,
        KeyCode::End => KeyAction::InputCursorEnd,
        KeyCode::Char(c) => KeyAction::InputChar(c),
        _ => KeyAction::Noop,
    }
}

fn map_key_viewer(key: KeyEvent) -> KeyAction {
    match key.code {
        KeyCode::Up => KeyAction::ViewerUp,
        KeyCode::Down => KeyAction::ViewerDown,
        KeyCode::PageUp => KeyAction::ViewerPageUp,
        KeyCode::PageDown => KeyAction::ViewerPageDown,
        KeyCode::Home => KeyAction::ViewerHome,
        KeyCode::End => KeyAction::ViewerEnd,
        KeyCode::Char('/') => KeyAction::ViewerSearch,
        KeyCode::Char('n') => KeyAction::ViewerNextMatch,
        KeyCode::Char('N') => KeyAction::ViewerPrevMatch,
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => KeyAction::ViewerClose,
        _ => KeyAction::Noop,
    }
}

fn map_key_viewer_search(key: KeyEvent) -> KeyAction {
    match key.code {
        KeyCode::Enter => KeyAction::ViewerSearchConfirm,
        KeyCode::Esc => KeyAction::ViewerSearchCancel,
        KeyCode::Backspace => KeyAction::ViewerSearchBackspace,
        KeyCode::Char(c) => KeyAction::ViewerSearchChar(c),
        _ => KeyAction::Noop,
    }
}

fn map_key_confirm(key: KeyEvent) -> KeyAction {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => KeyAction::ConfirmYes,
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => KeyAction::ConfirmNo,
        _ => KeyAction::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Up, KeyModifiers::NONE)),
            KeyAction::MoveUp
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Down, KeyModifiers::NONE)),
            KeyAction::MoveDown
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Left, KeyModifiers::NONE)),
            KeyAction::MoveLeft
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Right, KeyModifiers::NONE)),
            KeyAction::MoveRight
        );
    }

    #[test]
    fn test_quit_keys() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('q'), KeyModifiers::NONE)),
            KeyAction::Quit
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::F(10), KeyModifiers::NONE)),
            KeyAction::Quit
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            KeyAction::Quit
        );
    }

    #[test]
    fn test_navigation_keys() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Enter, KeyModifiers::NONE)),
            KeyAction::Enter
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Backspace, KeyModifiers::NONE)),
            KeyAction::Backspace
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Home, KeyModifiers::NONE)),
            KeyAction::Home
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::End, KeyModifiers::NONE)),
            KeyAction::End
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::PageUp, KeyModifiers::NONE)),
            KeyAction::PageUp
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::PageDown, KeyModifiers::NONE)),
            KeyAction::PageDown
        );
    }

    #[test]
    fn test_toggle_hidden() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('h'), KeyModifiers::NONE)),
            KeyAction::ToggleHidden
        );
    }

    #[test]
    fn test_unknown_key_noop() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('z'), KeyModifiers::NONE)),
            KeyAction::Noop
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('1'), KeyModifiers::NONE)),
            KeyAction::Noop
        );
    }

    // Phase 2 키 테스트

    #[test]
    fn test_select_key() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char(' '), KeyModifiers::NONE)),
            KeyAction::Select
        );
    }

    #[test]
    fn test_crud_keys() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('c'), KeyModifiers::NONE)),
            KeyAction::Copy
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('m'), KeyModifiers::NONE)),
            KeyAction::Move
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('d'), KeyModifiers::NONE)),
            KeyAction::Delete
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('r'), KeyModifiers::NONE)),
            KeyAction::Rename
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('k'), KeyModifiers::NONE)),
            KeyAction::Mkdir
        );
    }

    #[test]
    fn test_input_mode_keys() {
        assert_eq!(
            map_key_input(make_key(KeyCode::Char('a'), KeyModifiers::NONE)),
            KeyAction::InputChar('a')
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::Backspace, KeyModifiers::NONE)),
            KeyAction::InputBackspace
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::Enter, KeyModifiers::NONE)),
            KeyAction::InputConfirm
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::Esc, KeyModifiers::NONE)),
            KeyAction::InputCancel
        );
    }

    #[test]
    fn test_input_cursor_keys() {
        assert_eq!(
            map_key_input(make_key(KeyCode::Left, KeyModifiers::NONE)),
            KeyAction::InputCursorLeft
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::Right, KeyModifiers::NONE)),
            KeyAction::InputCursorRight
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::Home, KeyModifiers::NONE)),
            KeyAction::InputCursorHome
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::End, KeyModifiers::NONE)),
            KeyAction::InputCursorEnd
        );
        assert_eq!(
            map_key_input(make_key(KeyCode::Delete, KeyModifiers::NONE)),
            KeyAction::InputDelete
        );
    }

    #[test]
    fn test_confirm_mode_keys() {
        assert_eq!(
            map_key_confirm(make_key(KeyCode::Char('y'), KeyModifiers::NONE)),
            KeyAction::ConfirmYes
        );
        assert_eq!(
            map_key_confirm(make_key(KeyCode::Char('n'), KeyModifiers::NONE)),
            KeyAction::ConfirmNo
        );
        assert_eq!(
            map_key_confirm(make_key(KeyCode::Enter, KeyModifiers::NONE)),
            KeyAction::ConfirmYes
        );
        assert_eq!(
            map_key_confirm(make_key(KeyCode::Esc, KeyModifiers::NONE)),
            KeyAction::ConfirmNo
        );
    }

    // Phase 4 키 테스트

    #[test]
    fn test_view_and_search_keys() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('v'), KeyModifiers::NONE)),
            KeyAction::View
        );
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('f'), KeyModifiers::NONE)),
            KeyAction::FileSearch
        );
    }

    #[test]
    fn test_viewer_mode_keys() {
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Up, KeyModifiers::NONE)),
            KeyAction::ViewerUp
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Down, KeyModifiers::NONE)),
            KeyAction::ViewerDown
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::PageUp, KeyModifiers::NONE)),
            KeyAction::ViewerPageUp
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::PageDown, KeyModifiers::NONE)),
            KeyAction::ViewerPageDown
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Home, KeyModifiers::NONE)),
            KeyAction::ViewerHome
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::End, KeyModifiers::NONE)),
            KeyAction::ViewerEnd
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Char('/'), KeyModifiers::NONE)),
            KeyAction::ViewerSearch
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Char('n'), KeyModifiers::NONE)),
            KeyAction::ViewerNextMatch
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Char('N'), KeyModifiers::SHIFT)),
            KeyAction::ViewerPrevMatch
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Char('q'), KeyModifiers::NONE)),
            KeyAction::ViewerClose
        );
        assert_eq!(
            map_key_viewer(make_key(KeyCode::Esc, KeyModifiers::NONE)),
            KeyAction::ViewerClose
        );
    }

    #[test]
    fn test_viewer_search_mode_keys() {
        assert_eq!(
            map_key_viewer_search(make_key(KeyCode::Char('a'), KeyModifiers::NONE)),
            KeyAction::ViewerSearchChar('a')
        );
        assert_eq!(
            map_key_viewer_search(make_key(KeyCode::Backspace, KeyModifiers::NONE)),
            KeyAction::ViewerSearchBackspace
        );
        assert_eq!(
            map_key_viewer_search(make_key(KeyCode::Enter, KeyModifiers::NONE)),
            KeyAction::ViewerSearchConfirm
        );
        assert_eq!(
            map_key_viewer_search(make_key(KeyCode::Esc, KeyModifiers::NONE)),
            KeyAction::ViewerSearchCancel
        );
    }

    #[test]
    fn test_ctrl_c_not_copy() {
        assert_eq!(
            map_key_normal(make_key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            KeyAction::Quit
        );
    }
}
