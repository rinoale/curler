use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::KeyValueColumn;

const CELL_GAP: u16 = 1;
const MIN_KEY_CELL_WIDTH: u16 = 12;
const MAX_KEY_CELL_WIDTH: u16 = 24;
const MIN_VALUE_CELL_WIDTH: u16 = 6;

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

pub(super) fn lines_from_owned_value(
    key_title: &'static str,
    value_title: &'static str,
    rows: Vec<(&str, String)>,
    active: Option<(usize, KeyValueColumn)>,
    content_width: u16,
    add_label: &'static str,
) -> Vec<Line<'static>> {
    let layout = layout(content_width);
    let mut lines = Vec::new();
    let row_count = rows.len().max(1);

    for index in 0..row_count {
        let key = rows.get(index).map_or("", |(key, _)| *key);
        let value = rows.get(index).map_or("", |(_, value)| value.as_str());
        lines.extend(row_lines(
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

fn row_lines(
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

fn layout(content_width: u16) -> KeyValueLayout {
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

fn row_height(key: &str, value: &str, layout: KeyValueLayout) -> u16 {
    let content_height = wrap_cell_text(key, layout.key_inner_width())
        .len()
        .max(wrap_cell_text(value, layout.value_inner_width()).len())
        .max(1);

    (content_height as u16).saturating_add(2)
}

pub(super) fn total_height(rows: Vec<(&str, &str)>, content_width: u16) -> u16 {
    let layout = layout(content_width);
    let row_count = rows.len().max(1);
    let mut height: u16 = 0;

    for index in 0..row_count {
        let key = rows.get(index).map_or("", |(key, _)| *key);
        let value = rows.get(index).map_or("", |(_, value)| *value);
        height = height.saturating_add(row_height(key, value, layout));
    }

    height
}

pub(super) fn cell_at(
    content: Rect,
    title_rows: u16,
    rows: Vec<(&str, &str)>,
    column: u16,
    row: u16,
) -> Option<(usize, KeyValueColumn)> {
    let content = super::block_inner(content);
    let layout = layout(content.width);
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

    if (!super::contains(key, column, row) && !super::contains(value, column, row))
        || row < content.y.saturating_add(title_rows)
    {
        return None;
    }

    let row_count = rows.len().max(1);
    let mut row_start = content.y.saturating_add(title_rows);

    for index in 0..row_count {
        let key_text = rows.get(index).map_or("", |(key, _)| *key);
        let value_text = rows.get(index).map_or("", |(_, value)| *value);
        let row_height = row_height(key_text, value_text, layout);
        let row_end = row_start.saturating_add(row_height);

        if row >= row_start && row < row_end {
            if super::contains(key, column, row) {
                return Some((index, KeyValueColumn::Key));
            }
            if super::contains(value, column, row) {
                return Some((index, KeyValueColumn::Value));
            }
        }

        row_start = row_end;
    }

    None
}

pub(super) fn add_row_at(
    content: Rect,
    title_rows: u16,
    rows: Vec<(&str, &str)>,
    column: u16,
    row: u16,
) -> bool {
    let content = super::block_inner(content);
    if !super::contains(content, column, row) {
        return false;
    }

    let layout = layout(content.width);
    let row_count = rows.len().max(1);
    let mut row_start = content.y.saturating_add(title_rows);

    for index in 0..row_count {
        let key_text = rows.get(index).map_or("", |(key, _)| *key);
        let value_text = rows.get(index).map_or("", |(_, value)| *value);
        row_start = row_start.saturating_add(row_height(key_text, value_text, layout));
    }

    super::contains(
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

#[cfg(test)]
pub(super) fn value_offset(content_width: u16) -> u16 {
    layout(content_width).key_width.saturating_add(CELL_GAP)
}
