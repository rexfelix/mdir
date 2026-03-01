mod app;
mod event;
mod file_entry;
mod file_ops;
mod ui;

use app::App;
use crossterm::{
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    // panic 시 터미널 복원
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // 터미널 초기화
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 앱 초기화
    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
    let mut app = App::new(current_dir);

    // 초기 레이아웃 설정
    let size = terminal.size()?;
    app.update_layout(size.width, size.height);

    // 메인 이벤트 루프
    loop {
        terminal.draw(|frame| {
            ui::render(frame, &app);
        })?;

        match event::poll_event_with_mode(Duration::from_millis(100), app.input_mode())? {
            event::AppEvent::Key(action) => {
                app.handle_key(action);
            }
            event::AppEvent::Resize(w, h) => {
                app.update_layout(w, h);
            }
            event::AppEvent::None => {}
        }

        if app.should_quit {
            break;
        }
    }

    // 정상 종료 시 터미널 복원
    restore_terminal()?;
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    terminal::disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
