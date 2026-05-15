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
mod http;
mod project;
mod request;
mod state;
mod ui;

use app::{App, ContextTarget};

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn main() -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let import_args = match cli_action(&args) {
        CliAction::Run(import_args) => import_args,
        action => return handle_cli_action(action),
    };

    let app = App::load(import_args)?;
    let mut terminal = setup_terminal()?;
    let app_result = run_app(&mut terminal, app);
    let restore_result = restore_terminal(&mut terminal);

    restore_result?;
    app_result
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliAction {
    Run(Vec<String>),
    Help,
    Version,
}

fn cli_action(args: &[String]) -> CliAction {
    match args {
        [arg] if arg == "--help" || arg == "-h" => CliAction::Help,
        [arg] if arg == "--version" || arg == "-v" => CliAction::Version,
        _ => CliAction::Run(args.to_vec()),
    }
}

fn handle_cli_action(action: CliAction) -> io::Result<()> {
    match action {
        CliAction::Help => {
            print!("{}", help_text());
            Ok(())
        }
        CliAction::Version => {
            println!("curler {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliAction::Run(_) => Ok(()),
    }
}

fn help_text() -> String {
    format!(
        "\
curler {version}
Terminal HTTP client

USAGE:
    curler
    curler <url> [curl-compatible-options]

OPTIONS:
    -h, --help       Print help
    -v, --version    Print version

NOTES:
    Curler options are recognized only when they are the sole argument.
    Extra arguments are imported as request arguments.
",
        version = env!("CARGO_PKG_VERSION")
    )
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
    let mut resize_target = None;

    while !app.should_quit() {
        app.poll_request_runner();
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key_event(key);
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);

                    if app.overlay() == Some(app::Overlay::ContextMenu) {
                        if let Some(row_index) =
                            ui::context_menu_row_at(area, &app, mouse.column, mouse.row)
                        {
                            app.activate_context_menu_row(row_index);
                            continue;
                        }
                        app.close_overlay();
                    }

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
                            ui::method_menu_row_at(area, &app, mouse.column, mouse.row)
                    {
                        if let Some(method) = ui::method_for_menu_row(row_index) {
                            app.select_method_option(method);
                        }
                        continue;
                    }

                    if app.overlay() == Some(app::Overlay::BodyModeMenu)
                        && let Some(row_index) =
                            ui::body_mode_menu_row_at(area, &app, mouse.column, mouse.row)
                    {
                        if let Some(mode) = ui::body_mode_for_menu_row(row_index) {
                            app.select_body_mode_option(mode);
                        }
                        continue;
                    }

                    if let Some(target) = ui::resize_target_at(area, &app, mouse.column, mouse.row)
                    {
                        resize_target = Some(target);
                        apply_resize_target(&mut app, area, target, mouse.column, mouse.row);
                        continue;
                    }

                    if let Some(action) = ui::header_action_at(area, mouse.column, mouse.row) {
                        app.activate_header_action(action);
                        continue;
                    }

                    if ui::pane_at(area, &app, mouse.column, mouse.row)
                        == Some(app::FocusPane::Method)
                    {
                        app.set_focus(app::FocusPane::Method);
                        app.open_method_menu();
                        continue;
                    }

                    if ui::body_mode_control_at(area, &app, mouse.column, mouse.row) {
                        app.set_focus(app::FocusPane::Body);
                        app.open_body_mode_menu();
                        continue;
                    }

                    if ui::response_header_toggle_at(area, &app, mouse.column, mouse.row) {
                        app.toggle_response_headers();
                        continue;
                    }

                    if ui::local_header_add_row_at(area, &app, mouse.column, mouse.row) {
                        app.add_local_header_row();
                        continue;
                    }

                    if let Some((row_index, column)) =
                        ui::local_header_cell_at(area, &app, mouse.column, mouse.row)
                    {
                        app.select_local_header_cell(row_index, column);
                        continue;
                    }

                    if ui::shared_header_add_row_at(area, &app, mouse.column, mouse.row) {
                        app.add_shared_header_row();
                        continue;
                    }

                    if let Some((row_index, column)) =
                        ui::shared_header_cell_at(area, &app, mouse.column, mouse.row)
                    {
                        app.select_shared_header_cell(row_index, column);
                        continue;
                    }

                    if ui::body_field_add_row_at(area, &app, mouse.column, mouse.row) {
                        app.add_body_field_row();
                        continue;
                    }

                    if let Some((row_index, column)) =
                        ui::body_field_cell_at(area, &app, mouse.column, mouse.row)
                    {
                        app.select_body_field_cell(row_index, column);
                        continue;
                    }

                    if let Some(pane) = ui::pane_at(area, &app, mouse.column, mouse.row) {
                        app.set_focus(pane);
                    }

                    if let Some(row_index) = ui::history_row_at(area, &app, mouse.column, mouse.row)
                    {
                        app.activate_history_row(row_index);
                    }
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Right) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);

                    if let Some(target) = context_target_at(area, &app, mouse.column, mouse.row) {
                        app.open_context_menu(target, mouse.column, mouse.row);
                    }
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Drag(MouseButton::Left) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);

                    if let Some(target) = resize_target {
                        apply_resize_target(&mut app, area, target, mouse.column, mouse.row);
                    }
                }
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Up(MouseButton::Left) => {
                    resize_target = None;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn apply_resize_target(app: &mut App, area: Rect, target: ui::ResizeTarget, column: u16, row: u16) {
    match target {
        ui::ResizeTarget::History => {
            let width = ui::history_width_from_column(area, column);
            app.set_history_width(width);
        }
        ui::ResizeTarget::RequestHeight => {
            let height = ui::request_height_from_row(area, app, row);
            app.set_request_height(height);
        }
        ui::ResizeTarget::RequestMethod => {
            let width = ui::request_method_width_from_column(area, app, column);
            app.set_request_method_width(width);
        }
        ui::ResizeTarget::RequestUrl => {
            let width = ui::request_url_width_from_column(area, app, column);
            app.set_request_url_width(width);
        }
        ui::ResizeTarget::HeaderState => {
            let width = ui::editor_header_width_from_column(area, app, column);
            app.set_editor_headers_width(width);
        }
        ui::ResizeTarget::HeaderBody => {
            let height = ui::editor_header_height_from_row(area, app, row);
            app.set_editor_headers_height(height);
        }
        ui::ResizeTarget::BodyResponse => {
            let width = ui::body_width_from_column(area, app, column);
            app.set_body_width(width);
        }
    }
}

fn context_target_at(area: Rect, app: &App, column: u16, row: u16) -> Option<ContextTarget> {
    if let Some(row_index) = ui::history_row_at(area, app, column, row) {
        return Some(ContextTarget::History(row_index));
    }

    if ui::pane_at(area, app, column, row) == Some(app::FocusPane::Method) {
        return Some(ContextTarget::Method);
    }

    if let Some((row_index, _)) = ui::local_header_cell_at(area, app, column, row) {
        return Some(ContextTarget::LocalHeader(row_index));
    }

    if ui::local_header_add_row_at(area, app, column, row) {
        return Some(ContextTarget::LocalHeaders);
    }

    if let Some((row_index, _)) = ui::shared_header_cell_at(area, app, column, row) {
        return Some(ContextTarget::SharedHeader(row_index));
    }

    if ui::shared_header_add_row_at(area, app, column, row) {
        return Some(ContextTarget::SharedHeaders);
    }

    if let Some((row_index, _)) = ui::body_field_cell_at(area, app, column, row) {
        return Some(ContextTarget::BodyField(row_index));
    }

    if ui::body_field_add_row_at(area, app, column, row) {
        return Some(ContextTarget::BodyFields);
    }

    match ui::pane_at(area, app, column, row) {
        Some(app::FocusPane::Headers) => Some(ContextTarget::LocalHeaders),
        Some(app::FocusPane::State) => Some(ContextTarget::SharedHeaders),
        Some(app::FocusPane::Body) if app.body_mode().is_key_value_body() => {
            Some(ContextTarget::BodyFields)
        }
        Some(app::FocusPane::Body) => Some(ContextTarget::BodyRaw),
        Some(app::FocusPane::Logs) => Some(ContextTarget::Logs),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn cli_flags_work_when_they_are_the_only_argument() {
        assert_eq!(cli_action(&strings(&["--help"])), CliAction::Help);
        assert_eq!(cli_action(&strings(&["-h"])), CliAction::Help);
        assert_eq!(cli_action(&strings(&["--version"])), CliAction::Version);
        assert_eq!(cli_action(&strings(&["-v"])), CliAction::Version);
    }

    #[test]
    fn cli_flags_are_import_args_when_part_of_a_request() {
        assert_eq!(
            cli_action(&strings(&["-v", "https://example.com"])),
            CliAction::Run(strings(&["-v", "https://example.com"]))
        );
        assert_eq!(
            cli_action(&strings(&["https://example.com", "-h"])),
            CliAction::Run(strings(&["https://example.com", "-h"]))
        );
    }

    #[test]
    fn help_text_mentions_supported_flags() {
        let text = help_text();

        assert!(text.contains("--help"));
        assert!(text.contains("--version"));
        assert!(text.contains("curler <url> [curl-compatible-options]"));
        assert!(!text.contains("curler curl"));
    }
}
