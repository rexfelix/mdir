use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub enum AppEvent {
    Key(KeyAction),
    Resize(u16, u16),
    None,
}

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
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
    ToggleHidden,
    Quit,
    Noop,
}

pub fn poll_event(timeout: Duration) -> std::io::Result<AppEvent> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => Ok(AppEvent::Key(map_key(key))),
            Event::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
            _ => Ok(AppEvent::None),
        }
    } else {
        Ok(AppEvent::None)
    }
}

fn map_key(key: KeyEvent) -> KeyAction {
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
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
        KeyCode::F(10) => KeyAction::Quit,
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
            map_key(make_key(KeyCode::Up, KeyModifiers::NONE)),
            KeyAction::MoveUp
        );
        assert_eq!(
            map_key(make_key(KeyCode::Down, KeyModifiers::NONE)),
            KeyAction::MoveDown
        );
        assert_eq!(
            map_key(make_key(KeyCode::Left, KeyModifiers::NONE)),
            KeyAction::MoveLeft
        );
        assert_eq!(
            map_key(make_key(KeyCode::Right, KeyModifiers::NONE)),
            KeyAction::MoveRight
        );
    }

    #[test]
    fn test_quit_keys() {
        assert_eq!(
            map_key(make_key(KeyCode::Char('q'), KeyModifiers::NONE)),
            KeyAction::Quit
        );
        assert_eq!(
            map_key(make_key(KeyCode::F(10), KeyModifiers::NONE)),
            KeyAction::Quit
        );
        assert_eq!(
            map_key(make_key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            KeyAction::Quit
        );
    }

    #[test]
    fn test_navigation_keys() {
        assert_eq!(
            map_key(make_key(KeyCode::Enter, KeyModifiers::NONE)),
            KeyAction::Enter
        );
        assert_eq!(
            map_key(make_key(KeyCode::Backspace, KeyModifiers::NONE)),
            KeyAction::Backspace
        );
        assert_eq!(
            map_key(make_key(KeyCode::Home, KeyModifiers::NONE)),
            KeyAction::Home
        );
        assert_eq!(
            map_key(make_key(KeyCode::End, KeyModifiers::NONE)),
            KeyAction::End
        );
        assert_eq!(
            map_key(make_key(KeyCode::PageUp, KeyModifiers::NONE)),
            KeyAction::PageUp
        );
        assert_eq!(
            map_key(make_key(KeyCode::PageDown, KeyModifiers::NONE)),
            KeyAction::PageDown
        );
    }

    #[test]
    fn test_toggle_hidden() {
        assert_eq!(
            map_key(make_key(KeyCode::Char('h'), KeyModifiers::NONE)),
            KeyAction::ToggleHidden
        );
    }

    #[test]
    fn test_unknown_key_noop() {
        assert_eq!(
            map_key(make_key(KeyCode::Char('z'), KeyModifiers::NONE)),
            KeyAction::Noop
        );
        assert_eq!(
            map_key(make_key(KeyCode::Char('1'), KeyModifiers::NONE)),
            KeyAction::Noop
        );
        assert_eq!(
            map_key(make_key(KeyCode::Char(' '), KeyModifiers::NONE)),
            KeyAction::Noop
        );
    }
}
