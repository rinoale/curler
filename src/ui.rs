use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::{App, FocusPane, HeaderAction, HistoryRow, KeyValueColumn, Overlay},
    request::BodyMode,
};

const HISTORY_WIDTH: u16 = 36;
const CELL_GAP: u16 = 1;
const MIN_KEY_CELL_WIDTH: u16 = 12;
const MAX_KEY_CELL_WIDTH: u16 = 24;
const MIN_VALUE_CELL_WIDTH: u16 = 6;
const RESPONSE_HEADER_PREVIEW_LIMIT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyValueLayout {
    key_width: u16,
    value_width: u16,
}

impl KeyValueLayout {
    fn key_inner_width(self) -> u16 {
        self.key_width.saturating_sub(2).max(1)
    }

    fn value_inner_width(self) -> u16 {
        self.value_width.saturating_sub(2).max(1)
    }
}

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

fn key_value_lines_from_owned_value(
    key_title: &'static str,
    value_title: &'static str,
    rows: Vec<(&str, String)>,
    active: Option<(usize, KeyValueColumn)>,
    content_width: u16,
    add_label: &'static str,
) -> Vec<Line<'static>> {
    let layout = key_value_layout(content_width);
    let mut lines = Vec::new();
    let row_count = rows.len().max(1);

    for index in 0..row_count {
        let key = rows.get(index).map_or("", |(key, _)| *key);
        let value = rows.get(index).map_or("", |(_, value)| value.as_str());
        lines.extend(key_value_row_lines(
            key_title,
            value_title,
            key,
            value,
            layout,
            active.map(|(active_index, column)| (active_index == index, column)),
        ));
    }

    lines.push(Line::from(Span::styled(
        add_label,
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    lines
}

fn key_value_row_lines(
    key_title: &str,
    value_title: &str,
    key: &str,
    value: &str,
    layout: KeyValueLayout,
    active: Option<(bool, KeyValueColumn)>,
) -> Vec<Line<'static>> {
    let key_active = active == Some((true, KeyValueColumn::Key));
    let value_active = active == Some((true, KeyValueColumn::Value));
    let key_style = if key_active {
        active_cell_style()
    } else {
        Style::default()
    };
    let value_style = if value_active {
        active_cell_style()
    } else {
        Style::default()
    };
    let key_border_style = cell_border_style(key_active);
    let value_border_style = cell_border_style(value_active);
    let key_lines = wrap_cell_text(key, layout.key_inner_width());
    let value_lines = wrap_cell_text(value, layout.value_inner_width());
    let body_height = key_lines.len().max(value_lines.len()).max(1);
    let mut lines = Vec::with_capacity(body_height + 2);

    lines.push(cell_border_line(
        layout,
        key_title,
        value_title,
        key_border_style,
        value_border_style,
    ));

    for index in 0..body_height {
        lines.push(cell_content_line(
            layout,
            key_lines.get(index).map_or("", String::as_str),
            value_lines.get(index).map_or("", String::as_str),
            key_style,
            value_style,
            key_border_style,
            value_border_style,
        ));
    }

    lines.push(cell_border_line(
        layout,
        "",
        "",
        key_border_style,
        value_border_style,
    ));
    lines
}

fn active_cell_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

fn cell_border_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn cell_border_line(
    layout: KeyValueLayout,
    key_title: &str,
    value_title: &str,
    key_style: Style,
    value_style: Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(labeled_cell_border(layout.key_width, key_title), key_style),
        Span::raw(" ".repeat(usize::from(CELL_GAP))),
        Span::styled(
            labeled_cell_border(layout.value_width, value_title),
            value_style,
        ),
    ])
}

fn cell_content_line(
    layout: KeyValueLayout,
    key: &str,
    value: &str,
    key_style: Style,
    value_style: Style,
    key_border_style: Style,
    value_border_style: Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled("|", key_border_style),
        Span::styled(pad_cell_text(key, layout.key_inner_width()), key_style),
        Span::styled("|", key_border_style),
        Span::raw(" ".repeat(usize::from(CELL_GAP))),
        Span::styled("|", value_border_style),
        Span::styled(
            pad_cell_text(value, layout.value_inner_width()),
            value_style,
        ),
        Span::styled("|", value_border_style),
    ])
}

fn cell_border(width: u16) -> String {
    if width < 2 {
        return "+".repeat(usize::from(width));
    }

    format!("+{}+", "-".repeat(usize::from(width.saturating_sub(2))))
}

fn labeled_cell_border(width: u16, label: &str) -> String {
    if width < 4 || label.is_empty() {
        return cell_border(width);
    }

    let inner_width = usize::from(width.saturating_sub(2));
    let label = label.chars().take(inner_width).collect::<String>();
    let padding = inner_width.saturating_sub(label.chars().count());

    format!("+{}{}+", label, "-".repeat(padding))
}

fn key_value_layout(content_width: u16) -> KeyValueLayout {
    let available = content_width.saturating_sub(CELL_GAP);

    if available < 4 {
        return KeyValueLayout {
            key_width: content_width.max(2),
            value_width: 0,
        };
    }

    if available < MIN_KEY_CELL_WIDTH.saturating_add(MIN_VALUE_CELL_WIDTH) {
        let key_width = available / 2;
        let value_width = available.saturating_sub(key_width);

        return KeyValueLayout {
            key_width,
            value_width,
        };
    }

    let key_width = (available.saturating_mul(2) / 3)
        .clamp(MIN_KEY_CELL_WIDTH, MAX_KEY_CELL_WIDTH)
        .min(available.saturating_sub(MIN_VALUE_CELL_WIDTH));
    let value_width = available.saturating_sub(key_width);

    KeyValueLayout {
        key_width,
        value_width,
    }
}

fn wrap_cell_text(value: &str, width: u16) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut current = String::new();

    for character in value.chars() {
        if character == '\n' {
            lines.push(current);
            current = String::new();
            continue;
        }

        current.push(character);

        if current.chars().count() == width {
            lines.push(current);
            current = String::new();
        }
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }

    lines
}

fn pad_cell_text(value: &str, width: u16) -> String {
    let width = usize::from(width.max(1));
    let mut text = value.chars().take(width).collect::<String>();
    let padding = width.saturating_sub(text.chars().count());
    text.extend(std::iter::repeat_n(' ', padding));
    text
}

fn key_value_row_height(key: &str, value: &str, layout: KeyValueLayout) -> u16 {
    let content_height = wrap_cell_text(key, layout.key_inner_width())
        .len()
        .max(wrap_cell_text(value, layout.value_inner_width()).len())
        .max(1);

    (content_height as u16).saturating_add(2)
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
        Some(Overlay::MethodMenu) => draw_method_menu(frame, area, app.request().method.as_str()),
        Some(Overlay::BodyModeMenu) => draw_body_mode_menu(frame, area, app.body_mode()),
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
    let rect = centered_rect(area, 74, 18);
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

pub fn response_header_toggle_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    let Some(line_index) = response_header_toggle_line_index(app) else {
        return false;
    };
    let content = block_inner(app_layout(area).response);
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

    key_value_cell_at(app_layout(area).headers, 0, rows, column, row)
}

pub fn local_header_add_row_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    let rows = app
        .request()
        .headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_add_row_at(app_layout(area).headers, 0, rows, column, row)
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

    key_value_cell_at(app_layout(area).state, 1, rows, column, row)
}

pub fn shared_header_add_row_at(area: Rect, app: &App, column: u16, row: u16) -> bool {
    let rows = app
        .state()
        .shared_headers
        .iter()
        .map(|header| (header.name.as_str(), header.value.as_str()))
        .collect::<Vec<_>>();

    key_value_add_row_at(app_layout(area).state, 1, rows, column, row)
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

    key_value_cell_at(app_layout(area).body, 1, rows, column, row)
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

    key_value_add_row_at(app_layout(area).body, 1, rows, column, row)
}

pub fn history_row_at(area: Rect, column: u16, row: u16) -> Option<usize> {
    let content = block_inner(app_layout(area).history);

    if !contains(content, column, row) {
        return None;
    }

    Some(usize::from(row - content.y))
}

fn key_value_cell_at(
    pane: Rect,
    row_offset: u16,
    rows: Vec<(&str, &str)>,
    column: u16,
    row: u16,
) -> Option<(usize, KeyValueColumn)> {
    let content = block_inner(pane);
    let layout = key_value_layout(content.width);
    let key = Rect {
        x: content.x,
        y: content.y,
        width: layout.key_width,
        height: content.height,
    };
    let value_x = content
        .x
        .saturating_add(layout.key_width)
        .saturating_add(CELL_GAP);
    let value = Rect {
        x: value_x,
        y: content.y,
        width: layout.value_width,
        height: content.height,
    };

    if !contains(key, column, row) && !contains(value, column, row) {
        return None;
    }

    let row_count = rows.len().max(1);
    let mut row_start = content.y.saturating_add(row_offset);

    for index in 0..row_count {
        let key_text = rows.get(index).map_or("", |(key, _)| *key);
        let value_text = rows.get(index).map_or("", |(_, value)| *value);
        let row_height = key_value_row_height(key_text, value_text, layout);
        let row_end = row_start.saturating_add(row_height);

        if row >= row_start && row < row_end {
            return if contains(key, column, row) {
                Some((index, KeyValueColumn::Key))
            } else {
                Some((index, KeyValueColumn::Value))
            };
        }

        row_start = row_end;
    }

    None
}

fn key_value_add_row_at(
    pane: Rect,
    row_offset: u16,
    rows: Vec<(&str, &str)>,
    column: u16,
    row: u16,
) -> bool {
    let content = block_inner(pane);
    let layout = key_value_layout(content.width);

    if !contains(content, column, row) {
        return false;
    }

    let row_count = rows.len().max(1);
    let mut row_start = content.y.saturating_add(row_offset);

    for index in 0..row_count {
        let key_text = rows.get(index).map_or("", |(key, _)| *key);
        let value_text = rows.get(index).map_or("", |(_, value)| *value);
        row_start = row_start.saturating_add(key_value_row_height(key_text, value_text, layout));
    }

    contains(
        Rect {
            x: content.x,
            y: row_start,
            width: content.width,
            height: 1,
        },
        column,
        row,
    )
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
        Constraint::Length(4),
    ])
    .split(area);
    let columns = Layout::horizontal([Constraint::Length(HISTORY_WIDTH), Constraint::Min(40)])
        .split(sections[1]);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(7),
        Constraint::Min(4),
    ])
    .split(columns[1]);
    let request_panes = Layout::horizontal([
        Constraint::Length(24),
        Constraint::Percentage(48),
        Constraint::Percentage(52),
    ])
    .split(rows[0]);
    let header_state =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(rows[1]);
    let lower =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(rows[2]);

    AppLayout {
        header: sections[0],
        history: columns[0],
        method: request_panes[0],
        url: request_panes[1],
        query: request_panes[2],
        headers: header_state[0],
        state: header_state[1],
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

    #[test]
    fn maps_key_value_cells() {
        let area = Rect::new(0, 0, 80, 24);
        let mut app = App::new();
        app.select_body_mode_option(BodyMode::UrlEncoded);
        let layout = app_layout(area);
        let header = block_inner(layout.headers);
        let state = block_inner(layout.state);
        let body = block_inner(layout.body);
        let header_layout = key_value_layout(header.width);
        let value_offset = header_layout.key_width + CELL_GAP;

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

        let layout = app_layout(area);
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
        app.set_response_for_test(crate::http::HttpResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: (0..10)
                .map(|index| crate::request::Header::new(format!("X-Test-{index}"), "yes"))
                .collect(),
            body: String::new(),
            truncated: false,
        });

        let response = block_inner(app_layout(area).response);

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
