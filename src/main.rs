use std::{
    io::{self, Stdout, stdout},
    time::Duration,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};

mod app;
mod history;
mod project;
mod request;
mod state;
mod ui;

use app::App;

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn main() -> io::Result<()> {
    let import_args = std::env::args().skip(1).collect::<Vec<_>>();
    let app = App::load(import_args)?;

    let mut terminal = setup_terminal()?;
    let app_result = run_app(&mut terminal, app);
    let restore_result = restore_terminal(&mut terminal);

    restore_result?;
    app_result
}

fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    Ok(terminal)
}

fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn run_app(terminal: &mut Tui, mut app: App) -> io::Result<()> {
    while !app.should_quit() {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key_event(key);
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);

                    if app.overlay() == Some(app::Overlay::Help) {
                        if let Some(action) = ui::header_action_at(area, mouse.column, mouse.row) {
                            app.activate_header_action(action);
                        }
                        continue;
                    }

                    if app.overlay() == Some(app::Overlay::FileMenu)
                        && let Some(row_index) = ui::file_menu_row_at(area, mouse.column, mouse.row)
                    {
                        app.activate_file_menu_row(row_index);
                        continue;
                    }

                    if app.overlay() == Some(app::Overlay::MethodMenu)
                        && let Some(row_index) =
                            ui::method_menu_row_at(area, mouse.column, mouse.row)
                    {
                        if let Some(method) = ui::method_for_menu_row(row_index) {
                            app.select_method_option(method);
                        }
                        continue;
                    }

                    if app.overlay() == Some(app::Overlay::BodyModeMenu)
                        && let Some(row_index) =
                            ui::body_mode_menu_row_at(area, mouse.column, mouse.row)
                    {
                        if let Some(mode) = ui::body_mode_for_menu_row(row_index) {
                            app.select_body_mode_option(mode);
                        }
                        continue;
                    }

                    if let Some(action) = ui::header_action_at(area, mouse.column, mouse.row) {
                        app.activate_header_action(action);
                        continue;
                    }

                    if ui::pane_at(area, mouse.column, mouse.row) == Some(app::FocusPane::Method) {
                        app.set_focus(app::FocusPane::Method);
                        app.open_method_menu();
                        continue;
                    }

                    if ui::body_mode_control_at(area, mouse.column, mouse.row) {
                        app.set_focus(app::FocusPane::Body);
                        app.open_body_mode_menu();
                        continue;
                    }

                    if let Some(pane) = ui::pane_at(area, mouse.column, mouse.row) {
                        app.set_focus(pane);
                    }

                    if let Some(row_index) = ui::history_row_at(area, mouse.column, mouse.row) {
                        app.activate_history_row(row_index);
                    }
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Right) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);

                    if let Some(pane) = ui::pane_at(area, mouse.column, mouse.row) {
                        app.open_context_menu(pane);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}
