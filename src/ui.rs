use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let sections = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(3),
    ])
    .split(area);

    draw_header(frame, sections[0]);
    draw_workspace(frame, sections[1], app);
    draw_footer(frame, sections[2]);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            "curler",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Terminal HTTP client"),
    ]);

    frame.render_widget(
        Paragraph::new(title).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn draw_workspace(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::horizontal([Constraint::Length(28), Constraint::Min(40)]).split(area);

    draw_collection(frame, columns[0]);
    draw_request_editor(frame, columns[1], app);
}

fn draw_collection(frame: &mut Frame<'_>, area: Rect) {
    let items = [
        ListItem::new("GET  /health"),
        ListItem::new("POST /v1/messages"),
        ListItem::new("PUT  /users/:id"),
    ];

    frame.render_widget(
        List::new(items)
            .block(Block::default().title("Requests").borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

fn draw_request_editor(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Percentage(35),
        Constraint::Percentage(65),
    ])
    .split(area);

    let request = app.request();
    let request_line = Line::from(vec![
        Span::styled(
            request.method.as_str(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::raw(request.url.as_str()),
    ]);

    frame.render_widget(
        Paragraph::new(request_line).block(Block::default().title("Request").borders(Borders::ALL)),
        rows[0],
    );

    let headers = request
        .headers
        .iter()
        .map(|(name, value)| Line::from(vec![Span::raw(name), Span::raw(": "), Span::raw(value)]))
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(headers)
            .block(Block::default().title("Headers").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        rows[1],
    );

    let lower =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).split(rows[2]);

    let body = if request.body.is_empty() {
        "<empty>"
    } else {
        request.body.as_str()
    };

    frame.render_widget(
        Paragraph::new(body)
            .block(Block::default().title("Body").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        lower[0],
    );

    frame.render_widget(
        Paragraph::new("No response yet")
            .block(Block::default().title("Response").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray)),
        lower[1],
    );
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect) {
    let controls = Line::from(vec![
        Span::raw("q"),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
        Span::raw("   Esc"),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
        Span::raw("   Ctrl-C"),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(
        Paragraph::new(controls).block(Block::default().borders(Borders::ALL)),
        area,
    );
}
