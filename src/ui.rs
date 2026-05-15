use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane, HeaderAction, HistoryRow, Overlay},
    request::BodyMode,
};

const HISTORY_WIDTH: u16 = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MethodOption {
    method: &'static str,
    label: &'static str,
}

const METHOD_OPTIONS: &[MethodOption] = &[
    MethodOption {
        method: "GET",
        label: "GET",
    },
    MethodOption {
        method: "POST",
        label: "POST",
    },
    MethodOption {
        method: "PUT",
        label: "PUT",
    },
    MethodOption {
        method: "PATCH",
        label: "PATCH",
    },
    MethodOption {
        method: "DELETE",
        label: "DEL",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BodyModeOption {
    mode: BodyMode,
}

const BODY_MODE_OPTIONS: &[BodyModeOption] = &[
    BodyModeOption {
        mode: BodyMode::Raw,
    },
    BodyModeOption {
        mode: BodyMode::FormData,
    },
    BodyModeOption {
        mode: BodyMode::UrlEncoded,
    },
    BodyModeOption {
        mode: BodyMode::Binary,
    },
];

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let layout = app_layout(area);

    draw_header(frame, layout.header, app);
    draw_history(frame, layout.history, app);
    draw_request_editor(frame, layout, app);
    draw_logs(frame, layout.logs, app);
    draw_overlay(frame, area, app);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let header = header_layout(area);
    frame.render_widget(
        Paragraph::new(menu_line(app)).block(Block::default().title("Menu").borders(Borders::ALL)),
        header.menu_bar,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "Run",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(Block::default().title("Actions").borders(Borders::ALL)),
        header.action_bar,
    );
}

fn menu_line(app: &App) -> Line<'static> {
    Line::from(vec![
        menu_span("Curler", app.overlay() == Some(Overlay::About)),
        Span::raw("  "),
        menu_span("File", app.overlay() == Some(Overlay::FileMenu)),
        Span::raw("  "),
        menu_span("Help", app.overlay() == Some(Overlay::Help)),
    ])
}

fn menu_span(label: &'static str, active: bool) -> Span<'static> {
    let style = if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    Span::styled(label, style)
}

fn draw_history(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = app
        .history_rows()
        .into_iter()
        .enumerate()
        .map(|row| history_row_item(row, app))
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items)
            .block(focused_block("History", FocusPane::History, app))
            .style(Style::default().fg(Color::White)),
        area,
    );
}

fn history_row_item((index, row): (usize, HistoryRow), app: &App) -> ListItem<'static> {
    let row_style = if app.focus() == FocusPane::History && app.history_cursor() == index {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    match row {
        HistoryRow::Host { origin, expanded } => {
            let marker = if expanded { "[-] " } else { "[+] " };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    origin,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
            .style(row_style)
        }
        HistoryRow::Route {
            method,
            display_path,
            expanded,
            ..
        } => {
            let marker = if expanded { "[-] " } else { "[+] " };
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(marker, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    method,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::raw(display_path),
            ]))
            .style(row_style)
        }
        HistoryRow::Variant {
            label,
            run_count,
            selected,
            ..
        } => {
            let marker = if selected { "> " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            ListItem::new(Line::from(vec![
                Span::raw("    "),
                Span::styled(marker, Style::default().fg(Color::DarkGray)),
                Span::styled(label, style),
                Span::styled(
                    format!(" x{run_count}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
            .style(row_style)
        }
        HistoryRow::Empty => ListItem::new(Line::from(Span::styled(
            "No history yet",
            Style::default().fg(Color::DarkGray),
        )))
        .style(row_style),
    }
}

fn draw_request_editor(frame: &mut Frame<'_>, layout: AppLayout, app: &App) {
    let request = app.request();

    frame.render_widget(
        Paragraph::new(method_dropdown_label(request.method.as_str())).block(focused_block(
            "Method",
            FocusPane::Method,
            app,
        )),
        layout.method,
    );
    frame.render_widget(
        Paragraph::new(app.url_input())
            .block(focused_block("Host / Path", FocusPane::Url, app))
            .wrap(Wrap { trim: false }),
        layout.url,
    );
    frame.render_widget(
        Paragraph::new(app.query_input())
            .block(focused_block("Query", FocusPane::Query, app))
            .wrap(Wrap { trim: false }),
        layout.query,
    );

    frame.render_widget(
        Paragraph::new(app.headers_input())
            .block(focused_block("Headers", FocusPane::Headers, app))
            .wrap(Wrap { trim: false }),
        layout.headers,
    );

    draw_state(frame, layout.state, app);

    draw_body(frame, layout.body, app);

    frame.render_widget(
        Paragraph::new("No response yet")
            .block(focused_block("Response", FocusPane::Response, app))
            .style(Style::default().fg(Color::DarkGray)),
        layout.response,
    );
}

fn method_dropdown_label(method: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            method.to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  [v]"),
    ])
}

fn draw_state(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let state = app.state();
    let mut lines = Vec::new();

    for header in &state.shared_headers {
        lines.push(Line::from(vec![
            Span::styled("Header ", Style::default().fg(Color::DarkGray)),
            Span::raw(header.name.clone()),
            Span::raw(": "),
            Span::raw(state.resolve_value(&header.value)),
        ]));
    }

    for cookie in &state.cookies {
        lines.push(Line::from(vec![
            Span::styled("Cookie ", Style::default().fg(Color::DarkGray)),
            Span::raw(cookie.name.clone()),
            Span::raw("="),
            Span::raw(cookie.value.clone()),
        ]));
    }

    for (name, value) in &state.variables {
        let value = if value.is_empty() {
            "<empty>".to_string()
        } else {
            value.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("Var ", Style::default().fg(Color::DarkGray)),
            Span::raw(name.clone()),
            Span::raw(" = "),
            Span::raw(value),
        ]));
    }

    for binding in &state.response_bindings {
        lines.push(Line::from(vec![
            Span::styled("Bind ", Style::default().fg(Color::DarkGray)),
            Span::raw(binding.variable.clone()),
            Span::raw(" <- "),
            Span::raw(binding.json_path.clone()),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "<empty>",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block("Project State", FocusPane::State, app))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_logs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = app
        .logs()
        .iter()
        .rev()
        .take(3)
        .map(|log| {
            Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Yellow)),
                Span::raw(log.clone()),
            ])
        })
        .collect::<Vec<_>>();
    lines.reverse();

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "<no logs>",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block("Logs", FocusPane::Logs, app))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_overlay(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.overlay() {
        Some(Overlay::About) => draw_about(frame, area),
        Some(Overlay::FileMenu) => draw_file_menu(frame, area),
        Some(Overlay::MethodMenu) => draw_method_menu(frame, area, app.request().method.as_str()),
        Some(Overlay::BodyModeMenu) => draw_body_mode_menu(frame, area, app.body_mode()),
        Some(Overlay::RenameHistory) => draw_rename_history(frame, area, app),
        Some(Overlay::Help) => draw_help(frame, area),
        None => {}
    }
}

fn draw_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![
        body_mode_label(app.body_mode()),
        Line::from(Span::styled(
            "----------------",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    lines.extend(body_editor_lines(app.body_mode(), app.body_input()));

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block("Body", FocusPane::Body, app))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn body_mode_label(mode: BodyMode) -> Line<'static> {
    Line::from(vec![
        Span::styled("Mode ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            mode.label(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  [v]"),
    ])
}

fn body_editor_lines(mode: BodyMode, input: &str) -> Vec<Line<'static>> {
    match mode {
        BodyMode::Raw | BodyMode::Binary => {
            if input.is_empty() {
                Vec::new()
            } else {
                input
                    .lines()
                    .map(|line| Line::from(line.to_string()))
                    .collect()
            }
        }
        BodyMode::FormData | BodyMode::UrlEncoded => key_value_body_lines(input),
    }
}

fn key_value_body_lines(input: &str) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("Key", Style::default().fg(Color::Cyan)),
        Span::raw("                  "),
        Span::styled("Value", Style::default().fg(Color::Cyan)),
    ])];

    lines.extend(input.lines().map(|line| {
        if let Some((key, value)) = line.split_once('=') {
            Line::from(vec![
                Span::raw(pad_field(key.trim(), 20)),
                Span::raw(value.trim().to_string()),
            ])
        } else {
            Line::from(line.to_string())
        }
    }));

    lines
}

fn draw_about(frame: &mut Frame<'_>, area: Rect) {
    let rect = centered_rect(area, 48, 9);
    let authors = option_env!("CARGO_PKG_AUTHORS")
        .filter(|authors| !authors.is_empty())
        .unwrap_or("curler contributors");
    let lines = vec![
        Line::from(Span::styled(
            "Curler",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("Terminal HTTP client"),
        Line::from(""),
        Line::from(format!("Version: {}", env!("CARGO_PKG_VERSION"))),
        Line::from(format!("Author: {authors}")),
        Line::from(""),
        Line::from("Esc closes this pane."),
    ];

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title("About Curler").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn draw_file_menu(frame: &mut Frame<'_>, area: Rect) {
    let rect = file_menu_rect(area);
    let lines = vec![Line::from(vec![
        Span::styled("Save", Style::default().fg(Color::Yellow)),
        Span::raw("   ^S"),
    ])];

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("File").borders(Borders::ALL)),
        rect,
    );
}

fn draw_method_menu(frame: &mut Frame<'_>, area: Rect, selected: &str) {
    let rect = method_menu_rect(area);
    let lines = METHOD_OPTIONS
        .iter()
        .map(|option| {
            let marker = if option.method == selected {
                "> "
            } else {
                "  "
            };
            let style = if option.method == selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::DarkGray)),
                Span::styled(option.method.to_string(), style),
            ])
        })
        .collect::<Vec<_>>();

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Method").borders(Borders::ALL)),
        rect,
    );
}

fn draw_body_mode_menu(frame: &mut Frame<'_>, area: Rect, selected: BodyMode) {
    let rect = body_mode_menu_rect(area);
    let lines = BODY_MODE_OPTIONS
        .iter()
        .map(|option| {
            let marker = if option.mode == selected { "> " } else { "  " };
            let style = if option.mode == selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::DarkGray)),
                Span::styled(option.mode.label().to_string(), style),
            ])
        })
        .collect::<Vec<_>>();

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Body Mode").borders(Borders::ALL)),
        rect,
    );
}

fn draw_rename_history(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rect = centered_rect(area, 56, 7);
    let lines = vec![
        Line::from("Name"),
        Line::from(app.rename_input().to_string()),
        Line::from(""),
        Line::from("Enter saves. Empty name restores generated label. Esc cancels."),
    ];

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Rename History")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn draw_help(frame: &mut Frame<'_>, area: Rect) {
    let rect = centered_rect(area, 68, 17);
    let lines = vec![
        Line::from(Span::styled(
            "Global",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("^Q quit   ^R run   ^S save   ^P command palette"),
        Line::from("^H/^J/^K/^L move focus   Tab/Shift-Tab move focus"),
        Line::from(""),
        Line::from(Span::styled(
            "Local",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("History: j/k move, Enter or Space expand/select, a add, d delete, r rename"),
        Line::from("Method: Enter/Space opens dropdown, click option; 1-5 selects"),
        Line::from("Host/Path and Query: type text, Backspace deletes text"),
        Line::from("Headers and Body: type text, Enter newline, Backspace deletes text"),
        Line::from("Body mode: click Mode dropdown inside Body"),
        Line::from("Response: v bind variable, y copy"),
        Line::from("Logs: c clear"),
        Line::from(""),
        Line::from("Esc closes this help pane."),
    ];

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().title("Help").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn focused_block(title: &'static str, pane: FocusPane, app: &App) -> Block<'static> {
    let title = if app.focus() == pane {
        format!("> {title}")
    } else {
        title.to_string()
    };
    let border_style = if app.focus() == pane {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
}

pub fn pane_at(area: Rect, column: u16, row: u16) -> Option<FocusPane> {
    let layout = app_layout(area);

    for (pane, pane_area) in [
        (FocusPane::History, layout.history),
        (FocusPane::Method, layout.method),
        (FocusPane::Url, layout.url),
        (FocusPane::Query, layout.query),
        (FocusPane::Headers, layout.headers),
        (FocusPane::State, layout.state),
        (FocusPane::Body, layout.body),
        (FocusPane::Response, layout.response),
        (FocusPane::Logs, layout.logs),
    ] {
        if contains(pane_area, column, row) {
            return Some(pane);
        }
    }

    None
}

pub fn header_action_at(area: Rect, column: u16, row: u16) -> Option<HeaderAction> {
    let header = header_layout(app_layout(area).header);

    if contains(header.curler, column, row) {
        Some(HeaderAction::Curler)
    } else if contains(header.run, column, row) {
        Some(HeaderAction::Run)
    } else if contains(header.file, column, row) {
        Some(HeaderAction::File)
    } else if contains(header.help, column, row) {
        Some(HeaderAction::Help)
    } else {
        None
    }
}

pub fn file_menu_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(file_menu_rect(area));

    if !contains(content, column, row) {
        return None;
    }

    Some(usize::from(row - content.y))
}

pub fn method_menu_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(method_menu_rect(area));

    if !contains(content, column, row) {
        return None;
    }

    let row_index = usize::from(row - content.y);

    if row_index < METHOD_OPTIONS.len() {
        Some(row_index)
    } else {
        None
    }
}

pub fn method_for_menu_row(row_index: usize) -> Option<&'static str> {
    METHOD_OPTIONS.get(row_index).map(|option| option.method)
}

pub fn body_mode_control_at(area: Rect, column: u16, row: u16) -> bool {
    contains(body_mode_control_rect(area), column, row)
}

pub fn body_mode_menu_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(body_mode_menu_rect(area));

    if !contains(content, column, row) {
        return None;
    }

    let row_index = usize::from(row - content.y);

    if row_index < BODY_MODE_OPTIONS.len() {
        Some(row_index)
    } else {
        None
    }
}

pub fn body_mode_for_menu_row(row_index: usize) -> Option<BodyMode> {
    BODY_MODE_OPTIONS.get(row_index).map(|option| option.mode)
}

pub fn history_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(app_layout(area).history);

    if !contains(content, column, row) {
        return None;
    }

    Some(usize::from(row - content.y))
}

#[derive(Debug, Clone, Copy)]
struct AppLayout {
    header: Rect,
    history: Rect,
    method: Rect,
    url: Rect,
    query: Rect,
    headers: Rect,
    state: Rect,
    body: Rect,
    response: Rect,
    logs: Rect,
}

#[derive(Debug, Clone, Copy)]
struct HeaderLayout {
    menu_bar: Rect,
    action_bar: Rect,
    curler: Rect,
    run: Rect,
    file: Rect,
    help: Rect,
}

fn header_layout(area: Rect) -> HeaderLayout {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Length(3)]).split(area);
    let menu_content = block_inner(rows[0]);
    let action_content = block_inner(rows[1]);
    let curler = Rect {
        x: menu_content.x,
        y: menu_content.y,
        width: 6,
        height: 1,
    };
    let file = Rect {
        x: curler.x.saturating_add(curler.width + 2),
        y: menu_content.y,
        width: 4,
        height: 1,
    };
    let help = Rect {
        x: file.x.saturating_add(file.width + 2),
        y: menu_content.y,
        width: 4,
        height: 1,
    };
    let run = Rect {
        x: action_content.x,
        y: action_content.y,
        width: 3,
        height: 1,
    };

    HeaderLayout {
        menu_bar: rows[0],
        action_bar: rows[1],
        curler,
        run,
        file,
        help,
    }
}

fn app_layout(area: Rect) -> AppLayout {
    let sections = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(5),
    ])
    .split(area);
    let columns = Layout::horizontal([Constraint::Length(HISTORY_WIDTH), Constraint::Min(40)])
        .split(sections[1]);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Percentage(28),
        Constraint::Percentage(22),
        Constraint::Percentage(50),
    ])
    .split(columns[1]);
    let request_panes = Layout::horizontal([
        Constraint::Length(24),
        Constraint::Percentage(48),
        Constraint::Percentage(52),
    ])
    .split(rows[0]);
    let lower =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(rows[3]);

    AppLayout {
        header: sections[0],
        history: columns[0],
        method: request_panes[0],
        url: request_panes[1],
        query: request_panes[2],
        headers: rows[1],
        state: rows[2],
        body: lower[0],
        response: lower[1],
        logs: sections[2],
    }
}

fn file_menu_rect(area: Rect) -> Rect {
    let header = header_layout(app_layout(area).header);

    bounded_rect(
        area,
        Rect {
            x: header.file.x,
            y: header.menu_bar.y.saturating_add(header.menu_bar.height),
            width: 14,
            height: 3,
        },
    )
}

fn method_menu_rect(area: Rect) -> Rect {
    let method = app_layout(area).method;

    bounded_rect(
        area,
        Rect {
            x: method.x,
            y: method.y.saturating_add(method.height),
            width: method.width,
            height: (METHOD_OPTIONS.len() as u16).saturating_add(2),
        },
    )
}

fn body_mode_control_rect(area: Rect) -> Rect {
    let content = block_inner(app_layout(area).body);

    Rect {
        x: content.x,
        y: content.y,
        width: content.width.min(24),
        height: 1,
    }
}

fn body_mode_menu_rect(area: Rect) -> Rect {
    let control = body_mode_control_rect(area);

    bounded_rect(
        area,
        Rect {
            x: control.x,
            y: control.y.saturating_add(1),
            width: 20,
            height: (BODY_MODE_OPTIONS.len() as u16).saturating_add(2),
        },
    )
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4));
    let height = height.min(area.height.saturating_sub(2));

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn bounded_rect(area: Rect, rect: Rect) -> Rect {
    let width = rect.width.min(area.width);
    let height = rect.height.min(area.height);
    let max_x = area.x.saturating_add(area.width.saturating_sub(width));
    let max_y = area.y.saturating_add(area.height.saturating_sub(height));

    Rect {
        x: rect.x.min(max_x),
        y: rect.y.min(max_y),
        width,
        height,
    }
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && row >= area.y
        && column < area.x.saturating_add(area.width)
        && row < area.y.saturating_add(area.height)
}

fn block_inner(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

fn pad_field(value: &str, width: usize) -> String {
    let mut field = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    let padding = width.saturating_sub(field.chars().count());
    field.extend(std::iter::repeat_n(' ', padding));
    field
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_mouse_position_to_history_row() {
        let area = Rect::new(0, 0, 80, 24);

        assert_eq!(history_row_at(area, 1, 7), Some(0));
        assert_eq!(history_row_at(area, 1, 8), Some(1));
        assert_eq!(history_row_at(area, 0, 7), None);
        assert_eq!(history_row_at(area, 1, 6), None);
    }

    #[test]
    fn maps_mouse_position_to_focus_pane() {
        let area = Rect::new(0, 0, 80, 24);

        assert_eq!(pane_at(area, 1, 7), Some(FocusPane::History));
        assert_eq!(pane_at(area, 37, 7), Some(FocusPane::Method));
        assert_eq!(pane_at(area, 1, 20), Some(FocusPane::Logs));
    }

    #[test]
    fn maps_header_controls_to_actions() {
        let area = Rect::new(0, 0, 80, 24);

        assert_eq!(header_action_at(area, 1, 1), Some(HeaderAction::Curler));
        assert_eq!(header_action_at(area, 9, 1), Some(HeaderAction::File));
        assert_eq!(header_action_at(area, 15, 1), Some(HeaderAction::Help));
        assert_eq!(header_action_at(area, 1, 4), Some(HeaderAction::Run));
    }

    #[test]
    fn file_menu_stays_inside_terminal_bounds() {
        let area = Rect::new(0, 0, 80, 24);
        let rect = file_menu_rect(area);

        assert!(rect.x.saturating_add(rect.width) <= area.width);
        assert!(rect.y.saturating_add(rect.height) <= area.height);
    }

    #[test]
    fn maps_method_menu_rows_to_methods() {
        let area = Rect::new(0, 0, 80, 24);

        assert_eq!(method_menu_row_at(area, 37, 10), Some(0));
        assert_eq!(method_menu_row_at(area, 37, 11), Some(1));
        assert_eq!(method_for_menu_row(0), Some("GET"));
        assert_eq!(method_for_menu_row(4), Some("DELETE"));
        assert_eq!(method_for_menu_row(5), None);
    }

    #[test]
    fn maps_body_mode_control_and_menu_rows() {
        let area = Rect::new(0, 0, 80, 24);
        let control = body_mode_control_rect(area);
        let menu = body_mode_menu_rect(area);

        assert!(body_mode_control_at(area, control.x, control.y));
        assert_eq!(body_mode_menu_row_at(area, menu.x + 1, menu.y + 1), Some(0));
        assert_eq!(body_mode_for_menu_row(0), Some(BodyMode::Raw));
        assert_eq!(body_mode_for_menu_row(3), Some(BodyMode::Binary));
        assert_eq!(body_mode_for_menu_row(4), None);
    }
}
