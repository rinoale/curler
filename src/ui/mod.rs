use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane, HeaderAction, HistoryRow, KeyValueColumn, Overlay},
    domain::request::BodyMode,
};

mod key_value;

use key_value::{
    add_row_at as key_value_add_row_at, cell_at as key_value_cell_at,
    lines_from_owned_value as key_value_lines_from_owned_value,
    total_height as key_value_total_height,
};

const HISTORY_WIDTH: u16 = 36;
const MIN_HISTORY_WIDTH: u16 = 18;
const MIN_EDITOR_WIDTH: u16 = 40;
const RESPONSE_HEADER_PREVIEW_LIMIT: usize = 8;
const DEFAULT_REQUEST_HEIGHT: u16 = 3;
const MIN_REQUEST_HEIGHT: u16 = 3;
const DEFAULT_METHOD_WIDTH: u16 = 24;
const MIN_REQUEST_PANE_WIDTH: u16 = 12;
const DEFAULT_MIN_LOWER_EDITOR_HEIGHT: u16 = 4;
const DRAG_MIN_LOWER_EDITOR_HEIGHT: u16 = 2;
const MIN_HEADER_STATE_HEIGHT: u16 = 5;
const MIN_HEADER_STATE_COLUMN_WIDTH: u16 = 18;
const DEFAULT_LOCAL_HEADERS_PERCENT: u16 = 60;
const MIN_LOWER_COLUMN_WIDTH: u16 = 18;
const DEFAULT_BODY_PERCENT: u16 = 45;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeTarget {
    History,
    RequestHeight,
    RequestMethod,
    RequestUrl,
    HeaderState,
    HeaderBody,
    BodyResponse,
}

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let layout = app_layout(area, app);

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

    draw_headers(frame, layout.headers, app);

    draw_state(frame, layout.state, app);

    draw_body(frame, layout.body, app);

    draw_response(frame, layout.response, app);
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

fn draw_headers(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app
        .request()
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.to_string()))
        .collect::<Vec<_>>();
    let content_width = block_inner(area).width;

    frame.render_widget(
        Paragraph::new(key_value_lines_from_owned_value(
            "Name",
            "Value",
            rows,
            app.active_local_header_cell(),
            content_width,
            "+ Add Header",
        ))
        .block(focused_block("Local Headers", FocusPane::Headers, app))
        .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_state(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let state = app.state();
    let mut lines = vec![Line::from(Span::styled(
        "Shared Headers",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ))];

    let shared_header_rows = state
        .shared_headers
        .iter()
        .map(|header| (header.name.as_str(), state.resolve_value(&header.value)))
        .collect::<Vec<_>>();
    lines.extend(key_value_lines_from_owned_value(
        "Name",
        "Value",
        shared_header_rows,
        app.active_shared_header_cell(),
        block_inner(area).width,
        "+ Add Shared Header",
    ));

    lines.push(Line::from(""));

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

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block(
                "Project State / Shared Config",
                FocusPane::State,
                app,
            ))
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

fn draw_response(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = response_lines(app);
    let style = if app.response().is_some() {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block("Response", FocusPane::Response, app))
            .style(style)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn response_lines(app: &App) -> Vec<Line<'static>> {
    if let Some(run) = app.active_run() {
        return vec![
            Line::from(vec![
                Span::styled("Running ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    run.summary.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "The UI remains usable while the request runs.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
    }

    let Some(response) = app.response() else {
        return vec![Line::from("No response yet")];
    };

    let status_style = if (200..300).contains(&response.status) {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if response.status >= 400 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled("HTTP ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} {}", response.status, response.status_text),
                status_style,
            ),
        ]),
        Line::from(""),
    ];

    let shown_headers = if app.response_headers_expanded() {
        response.headers.len()
    } else {
        response.headers.len().min(RESPONSE_HEADER_PREVIEW_LIMIT)
    };

    for header in response.headers.iter().take(shown_headers) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", header.name),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(header.value.clone()),
        ]));
    }

    if response.headers.len() > RESPONSE_HEADER_PREVIEW_LIMIT {
        let label = if app.response_headers_expanded() {
            "[-] Hide headers".to_string()
        } else {
            format!(
                "[+] {} more headers",
                response.headers.len() - RESPONSE_HEADER_PREVIEW_LIMIT
            )
        };
        lines.push(Line::from(Span::styled(
            label,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::UNDERLINED),
        )));
    }

    if !response.headers.is_empty() {
        lines.push(Line::from(""));
    }

    let body_lines = if response.body.is_empty() {
        vec![Line::from(Span::styled(
            "<empty body>",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        response
            .body
            .lines()
            .take(120)
            .map(|line| Line::from(line.to_string()))
            .collect::<Vec<_>>()
    };

    lines.extend(body_lines);

    if response.truncated {
        lines.push(Line::from(Span::styled(
            "... response body truncated",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

fn draw_overlay(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.overlay() {
        Some(Overlay::About) => draw_about(frame, area),
        Some(Overlay::FileMenu) => draw_file_menu(frame, area),
        Some(Overlay::MethodMenu) => {
            draw_method_menu(frame, area, app, app.request().method.as_str())
        }
        Some(Overlay::BodyModeMenu) => draw_body_mode_menu(frame, area, app, app.body_mode()),
        Some(Overlay::RenameHistory) => draw_rename_history(frame, area, app),
        Some(Overlay::Help) => draw_help(frame, area),
        Some(Overlay::ContextMenu) => draw_context_menu(frame, area, app),
        None => {}
    }
}

fn draw_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![body_mode_label(app.body_mode())];

    lines.extend(body_editor_lines(app, block_inner(area).width));

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

fn body_editor_lines(app: &App, content_width: u16) -> Vec<Line<'static>> {
    match app.body_mode() {
        BodyMode::Raw | BodyMode::Binary => {
            let input = app.body_input();
            if input.is_empty() {
                Vec::new()
            } else {
                input
                    .lines()
                    .map(|line| Line::from(line.to_string()))
                    .collect()
            }
        }
        BodyMode::FormData | BodyMode::UrlEncoded => key_value_body_lines(app, content_width),
    }
}

fn key_value_body_lines(app: &App, content_width: u16) -> Vec<Line<'static>> {
    key_value_lines_from_owned_value(
        "Key",
        "Value",
        app.body_fields()
            .iter()
            .map(|field| (field.key.as_str(), field.value.clone()))
            .collect(),
        app.active_body_field_cell(),
        content_width,
        "+ Add Field",
    )
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

fn draw_method_menu(frame: &mut Frame<'_>, area: Rect, app: &App, selected: &str) {
    let rect = method_menu_rect(area, app);
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

fn draw_body_mode_menu(frame: &mut Frame<'_>, area: Rect, app: &App, selected: BodyMode) {
    let rect = body_mode_menu_rect(area, app);
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
    let rect = centered_rect(area, 78, 19);
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
        Line::from("Host/Path and Query: plain text input"),
        Line::from("Headers: key/value rows; raw Name: value paste is accepted"),
        Line::from("Body: Raw is text; Form/URL Encoded are key/value rows"),
        Line::from("Shared headers: edit rows in Project State / Shared Config"),
        Line::from("Body mode: click Mode dropdown inside Body"),
        Line::from("Response: click header toggle; h/Enter toggles headers; v bind, y copy"),
        Line::from("Layout: drag workspace pane borders; Menu, Actions, Logs are fixed"),
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

fn draw_context_menu(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(rect) = context_menu_rect(area, app) else {
        return;
    };

    let lines = app
        .context_menu_items()
        .into_iter()
        .map(|item| Line::from(Span::raw(item)))
        .collect::<Vec<_>>();

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().title("Menu").borders(Borders::ALL)),
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

pub fn pane_at(area: Rect, app: &App, column: u16, row: u16) -> Option<FocusPane> {
    let layout = app_layout(area, app);

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
    let header = header_layout(base_header_area(area));

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

pub fn method_menu_row_at(area: Rect, app: &App, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(method_menu_rect(area, app));

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

pub fn body_mode_control_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    contains(body_mode_control_rect(area, app), column, row)
}

pub fn body_mode_menu_row_at(area: Rect, app: &App, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(body_mode_menu_rect(area, app));

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

pub fn resize_target_at(area: Rect, app: &App, column: u16, row: u16) -> Option<ResizeTarget> {
    let layout = app_layout(area, app);
    let editor_right = layout.response.x.saturating_add(layout.response.width);
    let middle_top = layout.history.y;
    let middle_height = layout.history.height;

    let targets = [
        (
            ResizeTarget::RequestMethod,
            vertical_handle(layout.url.x, layout.method.y, layout.method.height),
        ),
        (
            ResizeTarget::RequestUrl,
            vertical_handle(layout.query.x, layout.url.y, layout.url.height),
        ),
        (
            ResizeTarget::RequestHeight,
            horizontal_handle(layout.headers.y, layout.method.x, editor_right),
        ),
        (
            ResizeTarget::HeaderState,
            vertical_handle(layout.state.x, layout.headers.y, layout.headers.height),
        ),
        (
            ResizeTarget::HeaderBody,
            horizontal_handle(layout.body.y, layout.method.x, editor_right),
        ),
        (
            ResizeTarget::BodyResponse,
            vertical_handle(layout.response.x, layout.body.y, layout.body.height),
        ),
        (
            ResizeTarget::History,
            vertical_handle(layout.method.x, middle_top, middle_height),
        ),
    ];

    targets
        .into_iter()
        .find_map(|(target, handle)| contains(handle, column, row).then_some(target))
}

pub fn history_width_from_column(area: Rect, column: u16) -> u16 {
    let middle = workspace_area(area);
    let min_width = history_min_width(middle.width);
    let max_width = middle.width.saturating_sub(MIN_EDITOR_WIDTH).max(min_width);

    column
        .saturating_sub(middle.x)
        .saturating_add(1)
        .clamp(min_width, max_width)
}

pub fn request_height_from_row(area: Rect, app: &App, row: u16) -> u16 {
    let editor = editor_area(area, app);
    let max_height = request_height_max(editor.height);

    row.saturating_sub(editor.y)
        .saturating_add(1)
        .clamp(MIN_REQUEST_HEIGHT.min(max_height), max_height)
}

pub fn request_method_width_from_column(area: Rect, app: &App, column: u16) -> u16 {
    let editor = editor_area(area, app);
    let max_width = editor
        .width
        .saturating_sub(MIN_REQUEST_PANE_WIDTH.saturating_mul(2));

    if max_width == 0 {
        return editor.width;
    }

    column
        .saturating_sub(editor.x)
        .saturating_add(1)
        .clamp(MIN_REQUEST_PANE_WIDTH.min(max_width), max_width)
}

pub fn request_url_width_from_column(area: Rect, app: &App, column: u16) -> u16 {
    let layout = app_layout(area, app);
    let remaining_width = layout.url.width.saturating_add(layout.query.width);
    let max_width = remaining_width.saturating_sub(MIN_REQUEST_PANE_WIDTH);

    if max_width == 0 {
        return remaining_width;
    }

    column
        .saturating_sub(layout.url.x)
        .saturating_add(1)
        .clamp(MIN_REQUEST_PANE_WIDTH.min(max_width), max_width)
}

pub fn editor_header_height_from_row(area: Rect, app: &App, row: u16) -> u16 {
    let editor = editor_area(area, app);
    let request_height = request_row_height(editor.height, app);
    let start = editor.y.saturating_add(request_height);
    let available = editor.height.saturating_sub(request_height);
    let max_height = available.saturating_sub(DRAG_MIN_LOWER_EDITOR_HEIGHT);

    if max_height == 0 {
        return available;
    }

    row.saturating_sub(start)
        .saturating_add(1)
        .clamp(MIN_HEADER_STATE_HEIGHT.min(max_height), max_height)
}

pub fn editor_header_width_from_column(area: Rect, app: &App, column: u16) -> u16 {
    let editor = editor_area(area, app);
    let min_width = header_state_column_min_width(editor.width);
    let max_width = editor.width.saturating_sub(min_width);

    if max_width == 0 {
        return editor.width;
    }

    column
        .saturating_sub(editor.x)
        .saturating_add(1)
        .clamp(min_width, max_width)
}

pub fn body_width_from_column(area: Rect, app: &App, column: u16) -> u16 {
    let layout = app_layout(area, app);
    let total_width = layout.body.width.saturating_add(layout.response.width);
    let min_width = lower_column_min_width(total_width);
    let max_width = total_width.saturating_sub(min_width);

    if max_width == 0 {
        return total_width;
    }

    column
        .saturating_sub(layout.body.x)
        .saturating_add(1)
        .clamp(min_width, max_width)
}

fn vertical_handle(x: u16, y: u16, height: u16) -> Rect {
    Rect {
        x: x.saturating_sub(1),
        y,
        width: 3,
        height,
    }
}

fn horizontal_handle(y: u16, x: u16, right: u16) -> Rect {
    Rect {
        x,
        y: y.saturating_sub(1),
        width: right.saturating_sub(x),
        height: 2,
    }
}

pub fn response_header_toggle_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    let Some(line_index) = response_header_toggle_line_index(app) else {
        return false;
    };
    let content = block_inner(app_layout(area, app).response);
    if line_index >= content.height {
        return false;
    }

    let toggle = Rect {
        x: content.x,
        y: content.y.saturating_add(line_index),
        width: content.width,
        height: 1,
    };

    contains(toggle, column, row)
}

pub fn context_menu_row_at(area: Rect, app: &App, column: u16, row: u16) -> Option<usize> {
    let item_count = app.context_menu_items().len();
    let content = block_inner(context_menu_rect(area, app)?);

    if !contains(content, column, row) {
        return None;
    }

    let row_index = usize::from(row - content.y);

    if row_index < item_count {
        Some(row_index)
    } else {
        None
    }
}

fn response_header_toggle_line_index(app: &App) -> Option<u16> {
    let response = app.response()?;

    if response.headers.len() <= RESPONSE_HEADER_PREVIEW_LIMIT {
        return None;
    }

    let shown_headers = if app.response_headers_expanded() {
        response.headers.len()
    } else {
        RESPONSE_HEADER_PREVIEW_LIMIT
    };

    Some(2_u16.saturating_add(shown_headers as u16))
}

pub fn local_header_cell_at(
    area: Rect,
    app: &App,
    column: u16,
    row: u16,
) -> Option<(usize, KeyValueColumn)> {
    let rows = app
        .request()
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_cell_at(app_layout(area, app).headers, 0, rows, column, row)
}

pub fn local_header_add_row_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    let rows = app
        .request()
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_add_row_at(app_layout(area, app).headers, 0, rows, column, row)
}

pub fn shared_header_cell_at(
    area: Rect,
    app: &App,
    column: u16,
    row: u16,
) -> Option<(usize, KeyValueColumn)> {
    let rows = app
        .state()
        .shared_headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_cell_at(app_layout(area, app).state, 1, rows, column, row)
}

pub fn shared_header_add_row_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    let rows = app
        .state()
        .shared_headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_add_row_at(app_layout(area, app).state, 1, rows, column, row)
}

pub fn body_field_cell_at(
    area: Rect,
    app: &App,
    column: u16,
    row: u16,
) -> Option<(usize, KeyValueColumn)> {
    if !app.body_mode().is_key_value_body() {
        return None;
    }

    let rows = app
        .body_fields()
        .iter()
        .map(|field| (field.key.as_str(), field.value.as_str()))
        .collect::<Vec<_>>();

    key_value_cell_at(app_layout(area, app).body, 1, rows, column, row)
}

pub fn body_field_add_row_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    if !app.body_mode().is_key_value_body() {
        return false;
    }

    let rows = app
        .body_fields()
        .iter()
        .map(|field| (field.key.as_str(), field.value.as_str()))
        .collect::<Vec<_>>();

    key_value_add_row_at(app_layout(area, app).body, 1, rows, column, row)
}

pub fn history_row_at(area: Rect, app: &App, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(app_layout(area, app).history);

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

fn app_layout(area: Rect, app: &App) -> AppLayout {
    let sections = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(4),
    ])
    .split(area);
    let history_width = history_width(sections[1].width, app);
    let columns = [
        Rect {
            x: sections[1].x,
            y: sections[1].y,
            width: history_width,
            height: sections[1].height,
        },
        Rect {
            x: sections[1].x.saturating_add(history_width),
            y: sections[1].y,
            width: sections[1].width.saturating_sub(history_width),
            height: sections[1].height,
        },
    ];
    let editor_area = columns[1];
    let request_height = request_row_height(editor_area.height, app);
    let below_request_height = editor_area.height.saturating_sub(request_height);
    let header_state_height =
        editor_header_state_height(editor_area.width, below_request_height, app);
    let lower_height = below_request_height.saturating_sub(header_state_height);
    let rows = Layout::vertical([
        Constraint::Length(request_height),
        Constraint::Length(header_state_height),
        Constraint::Length(lower_height),
    ])
    .split(editor_area);
    let (method, url, query) = request_columns(rows[0], app);
    let (headers, state) = header_state_columns(rows[1], app);
    let (body, response) = lower_columns(rows[2], app);

    AppLayout {
        header: sections[0],
        history: columns[0],
        method,
        url,
        query,
        headers,
        state,
        body,
        response,
        logs: sections[2],
    }
}

fn history_width(total_width: u16, app: &App) -> u16 {
    let min_width = history_min_width(total_width);
    let max_width = total_width.saturating_sub(MIN_EDITOR_WIDTH).max(min_width);

    app.history_width()
        .unwrap_or(HISTORY_WIDTH)
        .clamp(min_width, max_width)
}

fn history_min_width(total_width: u16) -> u16 {
    if total_width >= MIN_HISTORY_WIDTH.saturating_add(MIN_EDITOR_WIDTH) {
        MIN_HISTORY_WIDTH
    } else {
        total_width / 2
    }
}

fn request_row_height(editor_height: u16, app: &App) -> u16 {
    let max_height = request_height_max(editor_height);

    app.request_height()
        .unwrap_or(DEFAULT_REQUEST_HEIGHT)
        .clamp(MIN_REQUEST_HEIGHT.min(max_height), max_height)
}

fn request_height_max(editor_height: u16) -> u16 {
    let reserved = MIN_HEADER_STATE_HEIGHT.saturating_add(DRAG_MIN_LOWER_EDITOR_HEIGHT);
    editor_height
        .saturating_sub(reserved)
        .max(MIN_REQUEST_HEIGHT)
}

fn request_columns(area: Rect, app: &App) -> (Rect, Rect, Rect) {
    let method_width = request_method_width(area.width, app);
    let remaining_width = area.width.saturating_sub(method_width);
    let url_width = request_url_width(remaining_width, app);
    let query_width = remaining_width.saturating_sub(url_width);

    (
        Rect {
            x: area.x,
            y: area.y,
            width: method_width,
            height: area.height,
        },
        Rect {
            x: area.x.saturating_add(method_width),
            y: area.y,
            width: url_width,
            height: area.height,
        },
        Rect {
            x: area
                .x
                .saturating_add(method_width)
                .saturating_add(url_width),
            y: area.y,
            width: query_width,
            height: area.height,
        },
    )
}

fn request_method_width(total_width: u16, app: &App) -> u16 {
    let max_width = total_width.saturating_sub(MIN_REQUEST_PANE_WIDTH.saturating_mul(2));

    if max_width == 0 {
        return total_width / 3;
    }

    app.request_method_width()
        .unwrap_or(DEFAULT_METHOD_WIDTH)
        .clamp(MIN_REQUEST_PANE_WIDTH.min(max_width), max_width)
}

fn request_url_width(remaining_width: u16, app: &App) -> u16 {
    let max_width = remaining_width.saturating_sub(MIN_REQUEST_PANE_WIDTH);

    if max_width == 0 {
        return remaining_width / 2;
    }

    let default_width = remaining_width.saturating_mul(48) / 100;
    app.request_url_width()
        .unwrap_or(default_width)
        .clamp(MIN_REQUEST_PANE_WIDTH.min(max_width), max_width)
}

fn editor_header_state_height(editor_width: u16, available_height: u16, app: &App) -> u16 {
    if available_height == 0 {
        return 0;
    }

    let min_lower = if app.editor_headers_height().is_some() {
        DRAG_MIN_LOWER_EDITOR_HEIGHT
    } else {
        DEFAULT_MIN_LOWER_EDITOR_HEIGHT
    };
    let max_height = available_height.saturating_sub(min_lower);

    if max_height == 0 {
        return available_height;
    }

    if let Some(height) = app.editor_headers_height() {
        return height.clamp(MIN_HEADER_STATE_HEIGHT.min(max_height), max_height);
    }

    let local_width = local_headers_width(editor_width, app);
    let state_width = editor_width.saturating_sub(local_width);
    let local_content_width = block_inner(Rect {
        x: 0,
        y: 0,
        width: local_width,
        height: 1,
    })
    .width;
    let state_content_width = block_inner(Rect {
        x: 0,
        y: 0,
        width: state_width,
        height: 1,
    })
    .width;
    let local_desired = local_headers_desired_height(app, local_content_width);
    let state_desired = state_desired_height(app, state_content_width);
    let desired = local_desired.max(state_desired);

    desired.clamp(MIN_HEADER_STATE_HEIGHT.min(max_height), max_height)
}

fn local_headers_desired_height(app: &App, content_width: u16) -> u16 {
    let rows = app
        .request()
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_total_height(rows, content_width)
        .saturating_add(1)
        .saturating_add(2)
}

fn state_desired_height(app: &App, content_width: u16) -> u16 {
    let state = app.state();
    let rows = state
        .shared_headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();
    let extra_rows = state
        .cookies
        .len()
        .saturating_add(state.variables.len())
        .saturating_add(state.response_bindings.len());

    key_value_total_height(rows, content_width)
        .saturating_add(1)
        .saturating_add(1)
        .saturating_add(1)
        .saturating_add(extra_rows as u16)
        .saturating_add(2)
}

fn header_state_columns(area: Rect, app: &App) -> (Rect, Rect) {
    let left_width = local_headers_width(area.width, app);
    let right_width = area.width.saturating_sub(left_width);

    (
        Rect {
            x: area.x,
            y: area.y,
            width: left_width,
            height: area.height,
        },
        Rect {
            x: area.x.saturating_add(left_width),
            y: area.y,
            width: right_width,
            height: area.height,
        },
    )
}

fn lower_columns(area: Rect, app: &App) -> (Rect, Rect) {
    let left_width = body_width(area.width, app);
    let right_width = area.width.saturating_sub(left_width);

    (
        Rect {
            x: area.x,
            y: area.y,
            width: left_width,
            height: area.height,
        },
        Rect {
            x: area.x.saturating_add(left_width),
            y: area.y,
            width: right_width,
            height: area.height,
        },
    )
}

fn body_width(total_width: u16, app: &App) -> u16 {
    let min_width = lower_column_min_width(total_width);
    let max_width = total_width.saturating_sub(min_width);

    if max_width == 0 {
        return total_width;
    }

    let default_width = total_width.saturating_mul(DEFAULT_BODY_PERCENT) / 100;
    app.body_width()
        .unwrap_or(default_width)
        .clamp(min_width, max_width)
}

fn lower_column_min_width(total_width: u16) -> u16 {
    if total_width >= MIN_LOWER_COLUMN_WIDTH.saturating_mul(2) {
        MIN_LOWER_COLUMN_WIDTH
    } else {
        total_width / 2
    }
}

fn local_headers_width(total_width: u16, app: &App) -> u16 {
    let min_width = header_state_column_min_width(total_width);
    let max_width = total_width.saturating_sub(min_width);

    if max_width == 0 {
        return total_width;
    }

    let default_width = total_width.saturating_mul(DEFAULT_LOCAL_HEADERS_PERCENT) / 100;
    app.editor_headers_width()
        .unwrap_or(default_width)
        .clamp(min_width, max_width)
}

fn header_state_column_min_width(total_width: u16) -> u16 {
    if total_width >= MIN_HEADER_STATE_COLUMN_WIDTH.saturating_mul(2) {
        MIN_HEADER_STATE_COLUMN_WIDTH
    } else {
        total_width / 2
    }
}

fn base_header_area(area: Rect) -> Rect {
    Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(4),
    ])
    .split(area)[0]
}

fn workspace_area(area: Rect) -> Rect {
    Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(4),
    ])
    .split(area)[1]
}

fn editor_area(area: Rect, app: &App) -> Rect {
    let workspace = workspace_area(area);
    let history_width = history_width(workspace.width, app);

    Rect {
        x: workspace.x.saturating_add(history_width),
        y: workspace.y,
        width: workspace.width.saturating_sub(history_width),
        height: workspace.height,
    }
}

fn file_menu_rect(area: Rect) -> Rect {
    let header = header_layout(base_header_area(area));

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

fn method_menu_rect(area: Rect, app: &App) -> Rect {
    let method = app_layout(area, app).method;

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

fn body_mode_control_rect(area: Rect, app: &App) -> Rect {
    let content = block_inner(app_layout(area, app).body);

    Rect {
        x: content.x,
        y: content.y,
        width: content.width.min(24),
        height: 1,
    }
}

fn body_mode_menu_rect(area: Rect, app: &App) -> Rect {
    let control = body_mode_control_rect(area, app);

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

fn context_menu_rect(area: Rect, app: &App) -> Option<Rect> {
    let menu = app.context_menu()?;
    let items = app.context_menu_items();

    if items.is_empty() {
        return None;
    }

    let item_width = items
        .iter()
        .map(|item| item.chars().count())
        .max()
        .unwrap_or(0);
    let width = (item_width as u16).saturating_add(4).max(16);
    let height = (items.len() as u16).saturating_add(2);

    Some(bounded_rect(
        area,
        Rect {
            x: menu.column,
            y: menu.row,
            width,
            height,
        },
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_mouse_position_to_history_row() {
        let area = Rect::new(0, 0, 80, 24);
        let app = App::new();

        assert_eq!(history_row_at(area, &app, 1, 7), Some(0));
        assert_eq!(history_row_at(area, &app, 1, 8), Some(1));
        assert_eq!(history_row_at(area, &app, 0, 7), None);
        assert_eq!(history_row_at(area, &app, 1, 6), None);
    }

    #[test]
    fn maps_mouse_position_to_focus_pane() {
        let area = Rect::new(0, 0, 80, 24);
        let app = App::new();

        assert_eq!(pane_at(area, &app, 1, 7), Some(FocusPane::History));
        assert_eq!(pane_at(area, &app, 37, 7), Some(FocusPane::Method));
        assert_eq!(pane_at(area, &app, 1, 20), Some(FocusPane::Logs));
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
        let app = App::new();

        assert_eq!(method_menu_row_at(area, &app, 37, 10), Some(0));
        assert_eq!(method_menu_row_at(area, &app, 37, 11), Some(1));
        assert_eq!(method_for_menu_row(0), Some("GET"));
        assert_eq!(method_for_menu_row(4), Some("DELETE"));
        assert_eq!(method_for_menu_row(5), None);
    }

    #[test]
    fn maps_body_mode_control_and_menu_rows() {
        let area = Rect::new(0, 0, 80, 24);
        let app = App::new();
        let control = body_mode_control_rect(area, &app);
        let menu = body_mode_menu_rect(area, &app);

        assert!(body_mode_control_at(area, &app, control.x, control.y));
        assert_eq!(
            body_mode_menu_row_at(area, &app, menu.x + 1, menu.y + 1),
            Some(0)
        );
        assert_eq!(body_mode_for_menu_row(0), Some(BodyMode::Raw));
        assert_eq!(body_mode_for_menu_row(3), Some(BodyMode::Binary));
        assert_eq!(body_mode_for_menu_row(4), None);
    }

    #[test]
    fn editor_header_row_grows_with_header_content() {
        let area = Rect::new(0, 0, 120, 36);
        let mut app = App::new();
        let initial_height = app_layout(area, &app).headers.height;

        app.add_local_header_row();
        let expanded_height = app_layout(area, &app).headers.height;

        assert!(expanded_height > initial_height);
    }

    #[test]
    fn maps_editor_header_resize_handle_and_drag_height() {
        let area = Rect::new(0, 0, 120, 36);
        let mut app = App::new();
        let layout = app_layout(area, &app);
        let handle_y = layout.body.y.saturating_sub(1);

        assert_eq!(
            resize_target_at(area, &app, layout.method.x, handle_y),
            Some(ResizeTarget::HeaderBody)
        );

        let dragged_height = editor_header_height_from_row(area, &app, handle_y.saturating_add(3));
        app.set_editor_headers_height(dragged_height);

        assert_eq!(app_layout(area, &app).headers.height, dragged_height);
    }

    #[test]
    fn maps_editor_header_vertical_resize_handle_and_drag_width() {
        let area = Rect::new(0, 0, 120, 36);
        let mut app = App::new();
        let layout = app_layout(area, &app);
        let handle_x = layout.state.x;
        let handle_y = layout.headers.y.saturating_add(1);

        assert_eq!(
            resize_target_at(area, &app, handle_x, handle_y),
            Some(ResizeTarget::HeaderState)
        );

        let dragged_width = editor_header_width_from_column(area, &app, handle_x.saturating_add(5));
        app.set_editor_headers_width(dragged_width);

        assert_eq!(app_layout(area, &app).headers.width, dragged_width);
    }

    #[test]
    fn maps_workspace_dividers_to_resize_targets() {
        let area = Rect::new(0, 0, 120, 36);
        let app = App::new();
        let layout = app_layout(area, &app);

        assert_eq!(
            resize_target_at(area, &app, layout.method.x, layout.history.y + 1),
            Some(ResizeTarget::History)
        );
        assert_eq!(
            resize_target_at(area, &app, layout.url.x, layout.method.y + 1),
            Some(ResizeTarget::RequestMethod)
        );
        assert_eq!(
            resize_target_at(area, &app, layout.query.x, layout.url.y + 1),
            Some(ResizeTarget::RequestUrl)
        );
        assert_eq!(
            resize_target_at(area, &app, layout.headers.x, layout.headers.y),
            Some(ResizeTarget::RequestHeight)
        );
        assert_eq!(
            resize_target_at(area, &app, layout.state.x, layout.headers.y + 1),
            Some(ResizeTarget::HeaderState)
        );
        assert_eq!(
            resize_target_at(area, &app, layout.body.x, layout.body.y),
            Some(ResizeTarget::HeaderBody)
        );
        assert_eq!(
            resize_target_at(area, &app, layout.response.x, layout.body.y + 1),
            Some(ResizeTarget::BodyResponse)
        );
        assert_eq!(resize_target_at(area, &app, 1, 1), None);
        assert_eq!(resize_target_at(area, &app, 1, layout.logs.y), None);
    }

    #[test]
    fn maps_key_value_cells() {
        let area = Rect::new(0, 0, 80, 24);
        let mut app = App::new();
        app.select_body_mode_option(BodyMode::UrlEncoded);
        let layout = app_layout(area, &app);
        let header = block_inner(layout.headers);
        let state = block_inner(layout.state);
        let body = block_inner(layout.body);
        let value_offset = key_value::value_offset(header.width);

        assert_eq!(
            local_header_cell_at(area, &app, header.x, header.y),
            Some((0, KeyValueColumn::Key))
        );
        assert_eq!(
            local_header_cell_at(area, &app, header.x + value_offset, header.y),
            Some((0, KeyValueColumn::Value))
        );
        assert_eq!(
            shared_header_cell_at(area, &app, state.x, state.y + 1),
            Some((0, KeyValueColumn::Key))
        );
        assert_eq!(
            body_field_cell_at(area, &app, body.x, body.y + 1),
            Some((0, KeyValueColumn::Key))
        );
    }

    #[test]
    fn maps_key_value_add_rows() {
        let area = Rect::new(0, 0, 120, 36);
        let mut app = App::new();
        app.open_context_menu(crate::app::ContextTarget::LocalHeader(0), 0, 0);
        app.activate_context_menu_row(1);
        app.select_body_mode_option(BodyMode::UrlEncoded);

        let layout = app_layout(area, &app);
        let header = block_inner(layout.headers);
        let state = block_inner(layout.state);
        let body = block_inner(layout.body);

        assert!(local_header_add_row_at(area, &app, header.x, header.y + 3));
        assert!(shared_header_add_row_at(area, &app, state.x, state.y + 4));
        assert!(body_field_add_row_at(area, &app, body.x, body.y + 4));
    }

    #[test]
    fn maps_context_menu_rows() {
        let area = Rect::new(0, 0, 80, 24);
        let mut app = App::new();

        app.open_context_menu(crate::app::ContextTarget::LocalHeader(0), 20, 10);

        assert_eq!(context_menu_row_at(area, &app, 21, 11), Some(0));
        assert_eq!(context_menu_row_at(area, &app, 21, 12), Some(1));
        assert_eq!(context_menu_row_at(area, &app, 21, 13), Some(2));
        assert_eq!(context_menu_row_at(area, &app, 21, 14), None);
    }

    #[test]
    fn maps_response_header_toggle_line() {
        let area = Rect::new(0, 0, 120, 40);
        let mut app = App::new();
        app.set_response_for_test(crate::net::http::HttpResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: (0..10)
                .map(|index| crate::domain::request::Header::new(format!("X-Test-{index}"), "yes"))
                .collect(),
            body: String::new(),
            truncated: false,
        });

        let response = block_inner(app_layout(area, &app).response);

        assert!(response_header_toggle_at(
            area,
            &app,
            response.x,
            response.y + 10
        ));

        app.toggle_response_headers();

        assert!(response_header_toggle_at(
            area,
            &app,
            response.x,
            response.y + 12
        ));
    }
}
