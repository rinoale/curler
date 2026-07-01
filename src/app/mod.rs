use std::{collections::BTreeSet, io, sync::mpsc, thread};

#[cfg(test)]
use crossterm::event::KeyModifiers;
use crossterm::event::{KeyCode, KeyEvent};

mod keymap;

use keymap::{Intent, Keymap, text_input_modifiers};

use crate::{
    domain::{
        history::ProjectHistory,
        request::{BodyMode, Header, RequestDraft},
        state::ProjectState,
    },
    net::http::HttpResponse,
    storage::project::ProjectContext,
};

#[derive(Debug)]
pub struct App {
    should_quit: bool,
    project: Option<ProjectContext>,
    history: ProjectHistory,
    state: ProjectState,
    request: RequestDraft,
    url_input: String,
    query_input: String,
    headers_input: String,
    body_input: String,
    focus: FocusPane,
    keymap: Keymap,
    expanded_hosts: BTreeSet<String>,
    expanded_routes: BTreeSet<String>,
    selected_history_id: Option<String>,
    history_cursor: usize,
    overlay: Option<Overlay>,
    context_menu: Option<ContextMenuState>,
    active_editor: Option<EditorTarget>,
    body_fields: Vec<BodyField>,
    response: Option<HttpResponse>,
    response_headers_expanded: bool,
    history_width: Option<u16>,
    request_height: Option<u16>,
    request_method_width: Option<u16>,
    request_url_width: Option<u16>,
    editor_headers_height: Option<u16>,
    editor_headers_width: Option<u16>,
    body_width: Option<u16>,
    active_run: Option<ActiveRun>,
    next_run_id: u64,
    run_tx: mpsc::Sender<RunResult>,
    run_rx: mpsc::Receiver<RunResult>,
    rename_target_id: Option<String>,
    rename_input: String,
    logs: Vec<String>,
    status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    History,
    Method,
    Url,
    Query,
    Headers,
    State,
    Body,
    Response,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    About,
    FileMenu,
    MethodMenu,
    BodyModeMenu,
    RenameHistory,
    Help,
    ContextMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    Curler,
    Run,
    File,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValueColumn {
    Key,
    Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTarget {
    History(usize),
    Method,
    LocalHeader(usize),
    LocalHeaders,
    SharedHeader(usize),
    SharedHeaders,
    BodyField(usize),
    BodyFields,
    BodyRaw,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextMenuState {
    pub target: ContextTarget,
    pub column: u16,
    pub row: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRun {
    pub id: u64,
    pub summary: String,
}

#[derive(Debug)]
struct RunResult {
    id: u64,
    method: String,
    url: String,
    result: Result<HttpResponse, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Down,
    Up,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    RunRequest,
    SaveRequest,
    OpenCommandPalette,
    MoveFocus(Direction),
    Focus(FocusPane),
    HistoryUp,
    HistoryDown,
    ActivateHistory,
    SelectMethod(&'static str),
    OpenBodyModeMenu,
    SelectBodyMode(BodyMode),
    SubmitRename,
    ToggleResponseHeaders,
    OpenAbout,
    OpenFileMenu,
    OpenMethodMenu,
    OpenHelp,
    CloseOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryRow {
    Host {
        origin: String,
        expanded: bool,
    },
    Route {
        origin: String,
        method: String,
        path: String,
        display_path: String,
        expanded: bool,
    },
    Variant {
        id: String,
        label: String,
        run_count: u64,
        selected: bool,
    },
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorTarget {
    LocalHeader(usize, KeyValueColumn),
    SharedHeader(usize, KeyValueColumn),
    BodyField(usize, KeyValueColumn),
}

impl App {
    #[cfg(test)]
    pub fn new() -> Self {
        let request = RequestDraft::default();
        let (url_input, query_input, headers_input, body_input) =
            editor_inputs_from_request(&request);
        let body_fields = body_fields_from_input(request.body_mode, &body_input);
        let (run_tx, run_rx) = mpsc::channel();

        Self {
            should_quit: false,
            project: None,
            history: ProjectHistory::default(),
            state: ProjectState::default(),
            request,
            url_input,
            query_input,
            headers_input,
            body_input,
            focus: FocusPane::History,
            keymap: Keymap::default(),
            expanded_hosts: BTreeSet::new(),
            expanded_routes: BTreeSet::new(),
            selected_history_id: None,
            history_cursor: 0,
            overlay: None,
            context_menu: None,
            active_editor: None,
            body_fields,
            response: None,
            response_headers_expanded: false,
            history_width: None,
            request_height: None,
            request_method_width: None,
            request_url_width: None,
            editor_headers_height: None,
            editor_headers_width: None,
            body_width: None,
            active_run: None,
            next_run_id: 1,
            run_tx,
            run_rx,
            rename_target_id: None,
            rename_input: String::new(),
            logs: vec!["Ready".to_string()],
            status: "Ready".to_string(),
        }
    }

    pub fn load(import_args: Vec<String>) -> io::Result<Self> {
        let project = ProjectContext::discover()?;
        let mut history = ProjectHistory::load(&project.history_file, &project.root)?;
        let mut state = ProjectState::load(&project.state_file)?;
        let mut status = format!(
            "Project {}  {}",
            project.name,
            project.history_dir.display()
        );

        let mut selected_history_id = None;
        let request = if import_args.is_empty() {
            if let Some(entry) = history.latest() {
                selected_history_id = Some(entry.id.clone());
                entry.request.clone()
            } else {
                RequestDraft::default()
            }
        } else {
            match RequestDraft::from_curl_args(&import_args) {
                Ok(request) => {
                    selected_history_id = Some(history.upsert(request.clone()));
                    state.merge_from_request(&request);
                    history.save(&project.history_file)?;
                    state.save(&project.state_file)?;
                    status = format!("Imported {}", request.summary());
                    request
                }
                Err(error) => {
                    status = format!("Import failed: {error}");
                    if let Some(entry) = history.latest() {
                        selected_history_id = Some(entry.id.clone());
                        entry.request.clone()
                    } else {
                        RequestDraft::default()
                    }
                }
            }
        };

        let (url_input, query_input, headers_input, body_input) =
            editor_inputs_from_request(&request);
        let body_fields = body_fields_from_input(request.body_mode, &body_input);
        let (run_tx, run_rx) = mpsc::channel();
        let mut app = Self {
            should_quit: false,
            project: Some(project),
            history,
            state,
            request,
            url_input,
            query_input,
            headers_input,
            body_input,
            focus: FocusPane::History,
            keymap: Keymap::default(),
            expanded_hosts: BTreeSet::new(),
            expanded_routes: BTreeSet::new(),
            selected_history_id,
            history_cursor: 0,
            overlay: None,
            context_menu: None,
            active_editor: None,
            body_fields,
            response: None,
            response_headers_expanded: false,
            history_width: None,
            request_height: None,
            request_method_width: None,
            request_url_width: None,
            editor_headers_height: None,
            editor_headers_width: None,
            body_width: None,
            active_run: None,
            next_run_id: 1,
            run_tx,
            run_rx,
            rename_target_id: None,
            rename_input: String::new(),
            logs: vec![status.clone()],
            status,
        };
        app.expand_selected_history();

        Ok(app)
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn request(&self) -> &RequestDraft {
        &self.request
    }

    pub fn url_input(&self) -> &str {
        &self.url_input
    }

    pub fn query_input(&self) -> &str {
        &self.query_input
    }

    #[cfg(test)]
    pub fn headers_input(&self) -> &str {
        &self.headers_input
    }

    pub fn body_input(&self) -> &str {
        &self.body_input
    }

    pub fn body_mode(&self) -> BodyMode {
        self.request.body_mode
    }

    pub fn body_fields(&self) -> &[BodyField] {
        &self.body_fields
    }

    pub fn response(&self) -> Option<&HttpResponse> {
        self.response.as_ref()
    }

    pub fn response_headers_expanded(&self) -> bool {
        self.response_headers_expanded
    }

    pub fn active_run(&self) -> Option<&ActiveRun> {
        self.active_run.as_ref()
    }

    pub fn history_width(&self) -> Option<u16> {
        self.history_width
    }

    pub fn set_history_width(&mut self, width: u16) {
        self.history_width = Some(width);
    }

    pub fn request_height(&self) -> Option<u16> {
        self.request_height
    }

    pub fn set_request_height(&mut self, height: u16) {
        self.request_height = Some(height);
    }

    pub fn request_method_width(&self) -> Option<u16> {
        self.request_method_width
    }

    pub fn set_request_method_width(&mut self, width: u16) {
        self.request_method_width = Some(width);
    }

    pub fn request_url_width(&self) -> Option<u16> {
        self.request_url_width
    }

    pub fn set_request_url_width(&mut self, width: u16) {
        self.request_url_width = Some(width);
    }

    pub fn editor_headers_height(&self) -> Option<u16> {
        self.editor_headers_height
    }

    pub fn set_editor_headers_height(&mut self, height: u16) {
        self.editor_headers_height = Some(height);
    }

    pub fn editor_headers_width(&self) -> Option<u16> {
        self.editor_headers_width
    }

    pub fn set_editor_headers_width(&mut self, width: u16) {
        self.editor_headers_width = Some(width);
    }

    pub fn body_width(&self) -> Option<u16> {
        self.body_width
    }

    pub fn set_body_width(&mut self, width: u16) {
        self.body_width = Some(width);
    }

    #[cfg(test)]
    pub fn set_response_for_test(&mut self, response: HttpResponse) {
        self.response = Some(response);
        self.response_headers_expanded = false;
    }

    pub fn active_local_header_cell(&self) -> Option<(usize, KeyValueColumn)> {
        match self.active_editor {
            Some(EditorTarget::LocalHeader(index, column)) => Some((index, column)),
            _ => None,
        }
    }

    pub fn active_shared_header_cell(&self) -> Option<(usize, KeyValueColumn)> {
        match self.active_editor {
            Some(EditorTarget::SharedHeader(index, column)) => Some((index, column)),
            _ => None,
        }
    }

    pub fn active_body_field_cell(&self) -> Option<(usize, KeyValueColumn)> {
        match self.active_editor {
            Some(EditorTarget::BodyField(index, column)) => Some((index, column)),
            _ => None,
        }
    }

    pub fn rename_input(&self) -> &str {
        &self.rename_input
    }

    pub fn focus(&self) -> FocusPane {
        self.focus
    }

    pub fn state(&self) -> &ProjectState {
        &self.state
    }

    pub fn logs(&self) -> &[String] {
        &self.logs
    }

    pub fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    pub fn context_menu(&self) -> Option<&ContextMenuState> {
        self.context_menu.as_ref()
    }

    pub fn context_menu_items(&self) -> Vec<String> {
        let Some(menu) = self.context_menu else {
            return Vec::new();
        };

        match menu.target {
            ContextTarget::History(row_index) => self.history_context_menu_items(row_index),
            ContextTarget::Method => vec!["Select method".to_string()],
            ContextTarget::LocalHeader(_) => vec![
                "Add header below".to_string(),
                "Delete header".to_string(),
                "Clear header".to_string(),
            ],
            ContextTarget::LocalHeaders => vec!["Add header".to_string()],
            ContextTarget::SharedHeader(_) => vec![
                "Add shared header below".to_string(),
                "Delete shared header".to_string(),
                "Clear shared header".to_string(),
            ],
            ContextTarget::SharedHeaders => vec!["Add shared header".to_string()],
            ContextTarget::BodyField(_) => vec![
                "Add field below".to_string(),
                "Delete field".to_string(),
                "Clear field".to_string(),
            ],
            ContextTarget::BodyFields => vec!["Add field".to_string()],
            ContextTarget::BodyRaw => vec!["Clear body".to_string()],
            ContextTarget::Logs => vec!["Clear logs".to_string()],
        }
    }

    pub fn history_cursor(&self) -> usize {
        self.history_cursor
    }

    pub fn set_focus(&mut self, focus: FocusPane) {
        self.active_editor = None;
        if self.focus != focus {
            self.dispatch(Action::Focus(focus));
        }
    }

    pub fn open_context_menu(&mut self, target: ContextTarget, column: u16, row: u16) {
        self.context_menu = Some(ContextMenuState {
            target,
            column,
            row,
        });
        self.overlay = Some(Overlay::ContextMenu);
        self.set_focus_for_context_target(target);
        self.log("Context menu opened");
    }

    pub fn activate_context_menu_row(&mut self, row_index: usize) {
        let Some(menu) = self.context_menu else {
            return;
        };

        self.context_menu = None;
        self.overlay = None;

        match menu.target {
            ContextTarget::History(history_row) => {
                self.activate_history_context_menu_row(history_row, row_index)
            }
            ContextTarget::Method => {
                if row_index == 0 {
                    self.open_method_menu();
                }
            }
            ContextTarget::LocalHeader(index) => match row_index {
                0 => self.add_local_header_below(index),
                1 => self.delete_local_header(index),
                2 => self.clear_local_header(index),
                _ => {}
            },
            ContextTarget::LocalHeaders => {
                if row_index == 0 {
                    self.add_local_header_row();
                }
            }
            ContextTarget::SharedHeader(index) => match row_index {
                0 => self.add_shared_header_below(index),
                1 => self.delete_shared_header(index),
                2 => self.clear_shared_header(index),
                _ => {}
            },
            ContextTarget::SharedHeaders => {
                if row_index == 0 {
                    self.add_shared_header_row();
                }
            }
            ContextTarget::BodyField(index) => match row_index {
                0 => self.add_body_field_below(index),
                1 => self.delete_body_field(index),
                2 => self.clear_body_field(index),
                _ => {}
            },
            ContextTarget::BodyFields => {
                if row_index == 0 {
                    self.add_body_field_row();
                }
            }
            ContextTarget::BodyRaw => {
                if row_index == 0 {
                    self.clear_body();
                }
            }
            ContextTarget::Logs => {
                if row_index == 0 {
                    self.logs.clear();
                    self.log("Logs cleared");
                }
            }
        }
    }

    pub fn add_local_header_row(&mut self) {
        let index = self.request.headers.len();
        self.insert_local_header(index);
    }

    pub fn add_shared_header_row(&mut self) {
        let index = self.state.shared_headers.len();
        self.insert_shared_header(index);
    }

    pub fn add_body_field_row(&mut self) {
        let index = self.body_fields.len();
        self.insert_body_field(index);
    }

    pub fn activate_header_action(&mut self, action: HeaderAction) {
        match action {
            HeaderAction::Curler => self.dispatch(Action::OpenAbout),
            HeaderAction::Run => self.dispatch(Action::RunRequest),
            HeaderAction::File => self.dispatch(Action::OpenFileMenu),
            HeaderAction::Help => self.dispatch(Action::OpenHelp),
        }
    }

    pub fn activate_file_menu_row(&mut self, row_index: usize) {
        match row_index {
            0 => self.dispatch(Action::SaveRequest),
            _ => {}
        }
    }

    pub fn open_method_menu(&mut self) {
        self.dispatch(Action::OpenMethodMenu);
    }

    pub fn select_method_option(&mut self, method: &'static str) {
        self.dispatch(Action::SelectMethod(method));
    }

    pub fn open_body_mode_menu(&mut self) {
        self.dispatch(Action::OpenBodyModeMenu);
    }

    pub fn select_body_mode_option(&mut self, mode: BodyMode) {
        self.dispatch(Action::SelectBodyMode(mode));
    }

    pub fn close_overlay(&mut self) {
        self.dispatch(Action::CloseOverlay);
    }

    pub fn toggle_response_headers(&mut self) {
        self.dispatch(Action::ToggleResponseHeaders);
    }

    pub fn poll_request_runner(&mut self) {
        while let Ok(result) = self.run_rx.try_recv() {
            self.apply_run_result(result);
        }
    }

    pub fn select_local_header_cell(&mut self, index: usize, column: KeyValueColumn) {
        self.focus = FocusPane::Headers;
        ensure_header_row(&mut self.request.headers, index);
        self.sync_headers_input_from_request();
        self.active_editor = Some(EditorTarget::LocalHeader(index, column));
        self.log(format!("Editing local header {}", column.label()));
    }

    pub fn select_shared_header_cell(&mut self, index: usize, column: KeyValueColumn) {
        self.focus = FocusPane::State;
        ensure_header_row(&mut self.state.shared_headers, index);
        self.active_editor = Some(EditorTarget::SharedHeader(index, column));
        self.log(format!("Editing shared header {}", column.label()));
    }

    pub fn select_body_field_cell(&mut self, index: usize, column: KeyValueColumn) {
        if !self.request.body_mode.is_key_value_body() {
            self.focus = FocusPane::Body;
            self.active_editor = None;
            return;
        }

        self.focus = FocusPane::Body;
        ensure_body_field_row(&mut self.body_fields, index);
        self.sync_body_input_from_fields();
        self.active_editor = Some(EditorTarget::BodyField(index, column));
        self.log(format!("Editing body field {}", column.label()));
    }

    pub fn history_rows(&self) -> Vec<HistoryRow> {
        let mut rows = Vec::new();
        let mut last_origin = String::new();
        let mut last_route_key = String::new();

        let mut entries = self.history.entries.iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.origin
                .cmp(&right.origin)
                .then_with(|| left.method.cmp(&right.method))
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });

        for entry in entries {
            if entry.origin != last_origin {
                let expanded = self.expanded_hosts.contains(&entry.origin);
                rows.push(HistoryRow::Host {
                    origin: entry.origin.clone(),
                    expanded,
                });
                last_origin = entry.origin.clone();
                last_route_key.clear();
            }

            if !self.expanded_hosts.contains(&entry.origin) {
                continue;
            }

            let route_key = route_key(&entry.origin, &entry.method, &entry.path);
            if route_key != last_route_key {
                rows.push(HistoryRow::Route {
                    origin: entry.origin.clone(),
                    method: entry.method.clone(),
                    path: entry.path.clone(),
                    display_path: entry.request.display_path(),
                    expanded: self.expanded_routes.contains(&route_key),
                });
                last_route_key = route_key.clone();
            }

            if self.expanded_routes.contains(&route_key) {
                rows.push(HistoryRow::Variant {
                    id: entry.id.clone(),
                    label: entry
                        .name
                        .clone()
                        .unwrap_or_else(|| entry.request.variant_label()),
                    run_count: entry.run_count,
                    selected: self.selected_history_id.as_deref() == Some(entry.id.as_str()),
                });
            }
        }

        if rows.is_empty() {
            rows.push(HistoryRow::Empty);
        }

        rows
    }

    pub fn activate_history_row(&mut self, row_index: usize) {
        self.history_cursor = row_index.min(self.history_rows().len().saturating_sub(1));
        let Some(row) = self.history_rows().get(row_index).cloned() else {
            return;
        };

        match row {
            HistoryRow::Host { origin, expanded } => {
                if expanded {
                    self.expanded_hosts.remove(&origin);
                    self.log(format!("Collapsed {origin}"));
                } else {
                    self.expanded_hosts.insert(origin.clone());
                    self.log(format!("Expanded {origin}"));
                }
            }
            HistoryRow::Route {
                origin,
                method,
                path,
                display_path,
                expanded,
            } => {
                let key = route_key(&origin, &method, &path);

                if expanded {
                    self.expanded_routes.remove(&key);
                    self.log(format!("Collapsed {method} {display_path}"));
                } else {
                    self.expanded_hosts.insert(origin);
                    self.expanded_routes.insert(key);
                    self.log(format!("Expanded {method} {display_path}"));
                }
            }
            HistoryRow::Variant { id, .. } => self.select_history_entry(&id),
            HistoryRow::Empty => {}
        }
    }

    fn history_context_menu_items(&self, row_index: usize) -> Vec<String> {
        match self.history_rows().get(row_index) {
            Some(HistoryRow::Host { .. }) => {
                vec!["Add request".to_string(), "Delete host".to_string()]
            }
            Some(HistoryRow::Route { .. }) => {
                vec!["Add request".to_string(), "Delete path".to_string()]
            }
            Some(HistoryRow::Variant { .. }) => vec![
                "Add request".to_string(),
                "Rename history".to_string(),
                "Delete history".to_string(),
            ],
            Some(HistoryRow::Empty) | None => vec!["Add request".to_string()],
        }
    }

    fn set_focus_for_context_target(&mut self, target: ContextTarget) {
        self.active_editor = None;
        match target {
            ContextTarget::History(row_index) => {
                self.focus = FocusPane::History;
                self.history_cursor = row_index.min(self.history_rows().len().saturating_sub(1));
            }
            ContextTarget::Method => self.focus = FocusPane::Method,
            ContextTarget::LocalHeader(_) | ContextTarget::LocalHeaders => {
                self.focus = FocusPane::Headers
            }
            ContextTarget::SharedHeader(_) | ContextTarget::SharedHeaders => {
                self.focus = FocusPane::State
            }
            ContextTarget::BodyField(_) | ContextTarget::BodyFields | ContextTarget::BodyRaw => {
                self.focus = FocusPane::Body
            }
            ContextTarget::Logs => self.focus = FocusPane::Logs,
        }
    }

    fn activate_history_context_menu_row(&mut self, history_row: usize, menu_row: usize) {
        self.focus = FocusPane::History;
        self.history_cursor = history_row.min(self.history_rows().len().saturating_sub(1));

        let Some(row) = self.history_rows().get(self.history_cursor).cloned() else {
            return;
        };

        match row {
            HistoryRow::Host { .. } | HistoryRow::Route { .. } => match menu_row {
                0 => self.add_local_placeholder(),
                1 => self.delete_local_placeholder(),
                _ => {}
            },
            HistoryRow::Variant { .. } => match menu_row {
                0 => self.add_local_placeholder(),
                1 => self.rename_local_placeholder(),
                2 => self.delete_local_placeholder(),
                _ => {}
            },
            HistoryRow::Empty => {
                if menu_row == 0 {
                    self.add_local_placeholder();
                }
            }
        }
    }

    fn add_local_header_below(&mut self, index: usize) {
        let insert_at = if self.request.headers.is_empty() {
            0
        } else {
            index.saturating_add(1).min(self.request.headers.len())
        };
        self.insert_local_header(insert_at);
    }

    fn insert_local_header(&mut self, index: usize) {
        let insert_at = index.min(self.request.headers.len());
        self.request.headers.insert(insert_at, Header::new("", ""));
        self.sync_headers_input_from_request();
        self.focus = FocusPane::Headers;
        self.active_editor = Some(EditorTarget::LocalHeader(insert_at, KeyValueColumn::Key));
        self.log("Added local header");
    }

    fn delete_local_header(&mut self, index: usize) {
        if index >= self.request.headers.len() {
            self.log("No local header selected to delete");
            return;
        }

        self.request.headers.remove(index);
        self.sync_headers_input_from_request();
        self.active_editor = None;
        self.focus = FocusPane::Headers;
        self.log("Deleted local header");
    }

    fn clear_local_header(&mut self, index: usize) {
        ensure_header_row(&mut self.request.headers, index);
        self.request.headers[index] = Header::new("", "");
        self.sync_headers_input_from_request();
        self.active_editor = Some(EditorTarget::LocalHeader(index, KeyValueColumn::Key));
        self.focus = FocusPane::Headers;
        self.log("Cleared local header");
    }

    fn add_shared_header_below(&mut self, index: usize) {
        let insert_at = if self.state.shared_headers.is_empty() {
            0
        } else {
            index.saturating_add(1).min(self.state.shared_headers.len())
        };
        self.insert_shared_header(insert_at);
    }

    fn insert_shared_header(&mut self, index: usize) {
        let insert_at = index.min(self.state.shared_headers.len());
        self.state
            .shared_headers
            .insert(insert_at, Header::new("", ""));
        self.focus = FocusPane::State;
        self.active_editor = Some(EditorTarget::SharedHeader(insert_at, KeyValueColumn::Key));
        self.log("Added shared header");
    }

    fn delete_shared_header(&mut self, index: usize) {
        if index >= self.state.shared_headers.len() {
            self.log("No shared header selected to delete");
            return;
        }

        self.state.shared_headers.remove(index);
        self.active_editor = None;
        self.focus = FocusPane::State;
        self.log("Deleted shared header");
    }

    fn clear_shared_header(&mut self, index: usize) {
        ensure_header_row(&mut self.state.shared_headers, index);
        self.state.shared_headers[index] = Header::new("", "");
        self.active_editor = Some(EditorTarget::SharedHeader(index, KeyValueColumn::Key));
        self.focus = FocusPane::State;
        self.log("Cleared shared header");
    }

    fn add_body_field_below(&mut self, index: usize) {
        if !self.request.body_mode.is_key_value_body() {
            self.log("Body fields require Form Data or URL Encoded mode");
            return;
        }

        let insert_at = if self.body_fields.is_empty() {
            0
        } else {
            index.saturating_add(1).min(self.body_fields.len())
        };
        self.insert_body_field(insert_at);
    }

    fn insert_body_field(&mut self, index: usize) {
        if !self.request.body_mode.is_key_value_body() {
            self.log("Body fields require Form Data or URL Encoded mode");
            return;
        }

        let insert_at = index.min(self.body_fields.len());
        self.body_fields.insert(
            insert_at,
            BodyField {
                key: String::new(),
                value: String::new(),
            },
        );
        self.sync_body_input_from_fields();
        self.focus = FocusPane::Body;
        self.active_editor = Some(EditorTarget::BodyField(insert_at, KeyValueColumn::Key));
        self.log("Added body field");
    }

    fn delete_body_field(&mut self, index: usize) {
        if index >= self.body_fields.len() {
            self.log("No body field selected to delete");
            return;
        }

        self.body_fields.remove(index);
        self.sync_body_input_from_fields();
        self.active_editor = None;
        self.focus = FocusPane::Body;
        self.log("Deleted body field");
    }

    fn clear_body_field(&mut self, index: usize) {
        ensure_body_field_row(&mut self.body_fields, index);
        self.body_fields[index] = BodyField {
            key: String::new(),
            value: String::new(),
        };
        self.sync_body_input_from_fields();
        self.active_editor = Some(EditorTarget::BodyField(index, KeyValueColumn::Key));
        self.focus = FocusPane::Body;
        self.log("Cleared body field");
    }

    fn clear_body(&mut self) {
        self.body_input.clear();
        self.body_fields.clear();
        self.request.body.clear();
        self.active_editor = None;
        self.focus = FocusPane::Body;
        self.log("Cleared body");
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.overlay == Some(Overlay::RenameHistory) && self.handle_rename_key_event(key) {
            return;
        }

        if self.overlay.is_some() && self.keymap.intent_for(key) == Some(Intent::Cancel) {
            self.dispatch(Action::CloseOverlay);
            return;
        }

        if self.handle_text_key_event(key) {
            return;
        }

        if let Some(intent) = self.keymap.intent_for(key) {
            self.dispatch_intent(intent);
        }
    }

    fn dispatch_intent(&mut self, intent: Intent) {
        match intent {
            Intent::EnterCommandMode => self.dispatch(Action::OpenCommandPalette),
            Intent::Cancel => {
                if self.overlay.is_some() || self.context_menu.is_some() {
                    self.dispatch(Action::CloseOverlay);
                }
            }
            Intent::Help => self.dispatch(Action::OpenHelp),
            Intent::ToggleSafeMode => self.dispatch(Action::OpenBodyModeMenu),
            Intent::RefreshMetadata => self.dispatch(Action::RunRequest),
            Intent::ToggleFocus => self.dispatch(Action::MoveFocus(Direction::Right)),
            Intent::Submit => self.submit_focused(),
            Intent::Previous => self.previous_focused(),
            Intent::Next => self.next_focused(),
        }
    }

    fn submit_focused(&mut self) {
        match self.focus {
            FocusPane::History => self.dispatch(Action::ActivateHistory),
            FocusPane::Method => self.dispatch(Action::OpenMethodMenu),
            FocusPane::Response => self.dispatch(Action::ToggleResponseHeaders),
            FocusPane::Url
            | FocusPane::Query
            | FocusPane::Headers
            | FocusPane::State
            | FocusPane::Body => {
                self.dispatch(Action::RunRequest);
            }
            FocusPane::Logs => {}
        }
    }

    fn previous_focused(&mut self) {
        if self.focus == FocusPane::History {
            self.dispatch(Action::HistoryUp);
        } else {
            self.dispatch(Action::MoveFocus(Direction::Up));
        }
    }

    fn next_focused(&mut self) {
        if self.focus == FocusPane::History {
            self.dispatch(Action::HistoryDown);
        } else {
            self.dispatch(Action::MoveFocus(Direction::Down));
        }
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::RunRequest => self.run_current_request(),
            Action::SaveRequest => self.save_current_request(),
            Action::OpenCommandPalette => self.log("Command palette placeholder"),
            Action::MoveFocus(direction) => self.move_focus(direction),
            Action::Focus(focus) => {
                self.focus = focus;
                self.log(format!("Focused {}", focus.label()));
            }
            Action::HistoryUp => self.move_history_cursor(-1),
            Action::HistoryDown => self.move_history_cursor(1),
            Action::ActivateHistory => {
                if self.focus == FocusPane::History {
                    self.activate_history_row(self.history_cursor);
                }
            }
            Action::SelectMethod(method) => self.select_method(method),
            Action::OpenBodyModeMenu => self.open_body_mode_selector(),
            Action::SelectBodyMode(mode) => self.select_body_mode(mode),
            Action::SubmitRename => self.submit_history_rename(),
            Action::ToggleResponseHeaders => {
                if self.response.is_none() {
                    self.log("No response headers to expand");
                    return;
                }

                self.focus = FocusPane::Response;
                self.response_headers_expanded = !self.response_headers_expanded;
                if self.response_headers_expanded {
                    self.log("Expanded response headers");
                } else {
                    self.log("Collapsed response headers");
                }
            }
            Action::OpenAbout => {
                self.context_menu = None;
                self.overlay = Some(Overlay::About);
                self.log("Curler menu opened");
            }
            Action::OpenFileMenu => {
                self.context_menu = None;
                self.overlay = Some(Overlay::FileMenu);
                self.log("File menu opened");
            }
            Action::OpenMethodMenu => {
                self.context_menu = None;
                self.focus = FocusPane::Method;
                self.overlay = Some(Overlay::MethodMenu);
                self.log("Method menu opened");
            }
            Action::OpenHelp => {
                self.context_menu = None;
                self.overlay = Some(Overlay::Help);
                self.log("Help opened");
            }
            Action::CloseOverlay => {
                self.overlay = None;
                self.context_menu = None;
                self.rename_target_id = None;
                self.rename_input.clear();
                self.log("Overlay closed");
            }
        }
    }

    fn select_history_entry(&mut self, id: &str) {
        let Some(entry) = self
            .history
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            return;
        };

        let summary = entry.request.summary();
        self.selected_history_id = Some(entry.id);
        self.request = entry.request;
        self.sync_editor_inputs_from_request();
        self.expanded_hosts.insert(entry.origin.clone());
        self.expanded_routes
            .insert(route_key(&entry.origin, &entry.method, &entry.path));
        self.log(format!("Selected {summary}"));
    }

    fn expand_selected_history(&mut self) {
        if let Some(id) = self.selected_history_id.clone() {
            self.select_history_entry(&id);
        }
    }

    fn sync_editor_inputs_from_request(&mut self) {
        let (url_input, query_input, headers_input, body_input) =
            editor_inputs_from_request(&self.request);
        self.url_input = url_input;
        self.query_input = query_input;
        self.headers_input = headers_input;
        self.body_input = body_input;
        self.body_fields = body_fields_from_input(self.request.body_mode, &self.body_input);
    }

    fn sync_headers_input_from_request(&mut self) {
        self.headers_input = headers_input_from_request(&self.request);
    }

    fn sync_body_input_from_fields(&mut self) {
        if self.request.body_mode.is_key_value_body() {
            self.body_input = body_input_from_fields(self.request.body_mode, &self.body_fields);
            self.request.body.clone_from(&self.body_input);
        }
    }

    fn handle_rename_key_event(&mut self, key: KeyEvent) -> bool {
        if !text_input_modifiers(key.modifiers) {
            return false;
        }

        match key.code {
            KeyCode::Esc => {
                self.dispatch(Action::CloseOverlay);
                true
            }
            KeyCode::Enter => {
                self.dispatch(Action::SubmitRename);
                true
            }
            KeyCode::Char(character) => {
                self.rename_input.push(character);
                true
            }
            KeyCode::Backspace => {
                self.rename_input.pop();
                true
            }
            _ => false,
        }
    }

    fn handle_text_key_event(&mut self, key: KeyEvent) -> bool {
        if self.overlay.is_some() || !text_input_modifiers(key.modifiers) {
            return false;
        }

        if self.active_editor.is_some() && self.handle_key_value_key_event(key) {
            return true;
        }

        if !is_text_editor(self.focus) {
            return false;
        }

        let handled = match key.code {
            KeyCode::Char(character) => {
                self.push_editor_char(character);
                true
            }
            KeyCode::Backspace => {
                self.pop_editor_char();
                true
            }
            KeyCode::Enter => {
                if accepts_newline(self.focus) {
                    self.push_editor_char('\n');
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        if handled {
            self.try_sync_request_from_editor();
        }

        handled
    }

    fn handle_key_value_key_event(&mut self, key: KeyEvent) -> bool {
        let handled = match key.code {
            KeyCode::Char(character) => {
                self.push_key_value_char(character);
                true
            }
            KeyCode::Backspace => {
                self.pop_key_value_char();
                true
            }
            KeyCode::Enter => {
                self.advance_key_value_cell();
                true
            }
            _ => false,
        };

        if handled {
            self.sync_key_value_inputs();
        }

        handled
    }

    fn push_editor_char(&mut self, character: char) {
        match self.focus {
            FocusPane::Url => self.url_input.push(character),
            FocusPane::Query => self.query_input.push(character),
            FocusPane::Headers => self.headers_input.push(character),
            FocusPane::Body => self.body_input.push(character),
            _ => {}
        }
    }

    fn pop_editor_char(&mut self) {
        match self.focus {
            FocusPane::Url => {
                self.url_input.pop();
            }
            FocusPane::Query => {
                self.query_input.pop();
            }
            FocusPane::Headers => {
                self.headers_input.pop();
            }
            FocusPane::Body => {
                self.body_input.pop();
            }
            _ => {}
        }
    }

    fn push_key_value_char(&mut self, character: char) {
        match self.active_editor {
            Some(EditorTarget::LocalHeader(index, column)) => {
                ensure_header_row(&mut self.request.headers, index);
                push_header_cell(&mut self.request.headers[index], column, character);
            }
            Some(EditorTarget::SharedHeader(index, column)) => {
                ensure_header_row(&mut self.state.shared_headers, index);
                push_header_cell(&mut self.state.shared_headers[index], column, character);
            }
            Some(EditorTarget::BodyField(index, column)) => {
                ensure_body_field_row(&mut self.body_fields, index);
                push_body_field_cell(&mut self.body_fields[index], column, character);
            }
            None => {}
        }
    }

    fn pop_key_value_char(&mut self) {
        match self.active_editor {
            Some(EditorTarget::LocalHeader(index, column)) => {
                if let Some(header) = self.request.headers.get_mut(index) {
                    pop_header_cell(header, column);
                }
            }
            Some(EditorTarget::SharedHeader(index, column)) => {
                if let Some(header) = self.state.shared_headers.get_mut(index) {
                    pop_header_cell(header, column);
                }
            }
            Some(EditorTarget::BodyField(index, column)) => {
                if let Some(field) = self.body_fields.get_mut(index) {
                    pop_body_field_cell(field, column);
                }
            }
            None => {}
        }
    }

    fn advance_key_value_cell(&mut self) {
        self.active_editor = match self.active_editor {
            Some(EditorTarget::LocalHeader(index, KeyValueColumn::Key)) => {
                Some(EditorTarget::LocalHeader(index, KeyValueColumn::Value))
            }
            Some(EditorTarget::LocalHeader(index, KeyValueColumn::Value)) => {
                let next = index + 1;
                ensure_header_row(&mut self.request.headers, next);
                Some(EditorTarget::LocalHeader(next, KeyValueColumn::Key))
            }
            Some(EditorTarget::SharedHeader(index, KeyValueColumn::Key)) => {
                Some(EditorTarget::SharedHeader(index, KeyValueColumn::Value))
            }
            Some(EditorTarget::SharedHeader(index, KeyValueColumn::Value)) => {
                let next = index + 1;
                ensure_header_row(&mut self.state.shared_headers, next);
                Some(EditorTarget::SharedHeader(next, KeyValueColumn::Key))
            }
            Some(EditorTarget::BodyField(index, KeyValueColumn::Key)) => {
                Some(EditorTarget::BodyField(index, KeyValueColumn::Value))
            }
            Some(EditorTarget::BodyField(index, KeyValueColumn::Value)) => {
                let next = index + 1;
                ensure_body_field_row(&mut self.body_fields, next);
                Some(EditorTarget::BodyField(next, KeyValueColumn::Key))
            }
            None => None,
        };
    }

    fn sync_key_value_inputs(&mut self) {
        match self.active_editor {
            Some(EditorTarget::LocalHeader(_, _)) => self.sync_headers_input_from_request(),
            Some(EditorTarget::SharedHeader(_, _)) => {}
            Some(EditorTarget::BodyField(_, _)) => self.sync_body_input_from_fields(),
            None => {}
        }
    }

    fn try_sync_request_from_editor(&mut self) {
        let _ = self.sync_request_from_editor();
    }

    fn sync_request_from_editor(&mut self) -> Result<(), String> {
        self.request.set_url(&self.url_input)?;
        self.request.set_query(&self.query_input);
        self.request.headers = parse_headers_input(&self.headers_input);
        if self.request.body_mode.is_key_value_body() {
            self.body_fields = body_fields_from_input(self.request.body_mode, &self.body_input);
        }
        self.request.body.clone_from(&self.body_input);

        Ok(())
    }

    fn move_focus(&mut self, direction: Direction) {
        self.focus = match (self.focus, direction) {
            (FocusPane::History, Direction::Right) => FocusPane::Method,
            (FocusPane::History, Direction::Down) => FocusPane::Logs,
            (FocusPane::Method, Direction::Right) => FocusPane::Url,
            (FocusPane::Method, Direction::Down) => FocusPane::Headers,
            (FocusPane::Url, Direction::Right) => FocusPane::Query,
            (FocusPane::Url, Direction::Down) => FocusPane::Headers,
            (FocusPane::Query, Direction::Down) => FocusPane::Headers,
            (FocusPane::Headers, Direction::Up) => FocusPane::Method,
            (FocusPane::Headers, Direction::Down) => FocusPane::State,
            (FocusPane::State, Direction::Up) => FocusPane::Headers,
            (FocusPane::State, Direction::Down) => FocusPane::Body,
            (FocusPane::Body, Direction::Right) => FocusPane::Response,
            (FocusPane::Body, Direction::Up) => FocusPane::State,
            (FocusPane::Body, Direction::Down) => FocusPane::Logs,
            (FocusPane::Response, Direction::Up) => FocusPane::State,
            (FocusPane::Response, Direction::Down) => FocusPane::Logs,
            (FocusPane::Logs, Direction::Up) => FocusPane::Body,
            (focus, _) => focus,
        };
        self.log(format!("Focused {}", self.focus.label()));
    }

    fn move_history_cursor(&mut self, offset: isize) {
        let row_count = self.history_rows().len();
        if row_count == 0 {
            self.history_cursor = 0;
            return;
        }

        self.history_cursor = self
            .history_cursor
            .saturating_add_signed(offset)
            .min(row_count.saturating_sub(1));
    }

    fn save_current_request(&mut self) {
        self.overlay = None;
        self.context_menu = None;
        if self.persist_current_request() {
            self.log("Saved current request");
        }
    }

    fn run_current_request(&mut self) {
        self.overlay = None;
        self.context_menu = None;
        if !self.persist_current_request() {
            return;
        }

        let id = self.next_run_id;
        self.next_run_id = self.next_run_id.saturating_add(1);
        let request = self.request.clone();
        let state = self.state.clone();
        let method = request.method.clone();
        let url = request.url.clone();
        let summary = format!("{method} {url}");
        let tx = self.run_tx.clone();

        self.response_headers_expanded = false;
        self.response = None;
        self.active_run = Some(ActiveRun {
            id,
            summary: summary.clone(),
        });
        self.log(format!("Running {summary}"));

        thread::spawn(move || {
            let result = execute_request(&request, &state);
            let _ = tx.send(RunResult {
                id,
                method,
                url,
                result,
            });
        });
    }

    fn apply_run_result(&mut self, run_result: RunResult) {
        if self.active_run.as_ref().map(|run| run.id) != Some(run_result.id) {
            return;
        }

        self.active_run = None;

        match run_result.result {
            Ok(response) => {
                let summary = response.summary(&run_result.method, &run_result.url);
                if self.state.apply_response_body(&response.body).is_ok() {
                    self.save_state();
                }
                self.response_headers_expanded = false;
                self.response = Some(response);
                self.log(summary);
            }
            Err(error) => {
                self.response_headers_expanded = false;
                self.response = None;
                self.log(format!("Request failed: {error}"));
            }
        }
    }

    fn persist_current_request(&mut self) -> bool {
        if let Err(error) = self.sync_request_from_editor() {
            self.log(format!("Request edit invalid: {error}"));
            return false;
        }

        let id = self.history.upsert(self.request.clone());
        self.selected_history_id = Some(id);
        self.state.merge_from_request(&self.request);
        clean_empty_headers(&mut self.state.shared_headers);

        if let Some(project) = &self.project {
            if let Err(error) = self.history.save(&project.history_file) {
                self.log(format!("Save history failed: {error}"));
                return false;
            }

            if let Err(error) = self.state.save(&project.state_file) {
                self.log(format!("Save state failed: {error}"));
                return false;
            }
        }

        self.expand_selected_history();
        true
    }

    fn save_history(&mut self) -> bool {
        if let Some(project) = &self.project
            && let Err(error) = self.history.save(&project.history_file)
        {
            self.log(format!("Save history failed: {error}"));
            return false;
        }

        true
    }

    fn save_state(&mut self) -> bool {
        if let Some(project) = &self.project
            && let Err(error) = self.state.save(&project.state_file)
        {
            self.log(format!("Save state failed: {error}"));
            return false;
        }

        true
    }

    fn refresh_after_history_change(&mut self) {
        let selected_still_exists = self
            .selected_history_id
            .as_deref()
            .is_some_and(|id| self.history.entries.iter().any(|entry| entry.id == id));

        if !selected_still_exists {
            if let Some(entry) = self.history.latest().cloned() {
                self.selected_history_id = Some(entry.id.clone());
                self.request = entry.request;
            } else {
                self.selected_history_id = None;
                self.request = RequestDraft::default();
            }
            self.sync_editor_inputs_from_request();
        }

        let row_count = self.history_rows().len();
        self.history_cursor = self.history_cursor.min(row_count.saturating_sub(1));
    }

    fn add_local_placeholder(&mut self) {
        let message = match self.focus {
            FocusPane::History => "Add host/path/request placeholder",
            FocusPane::Headers => "Add header placeholder",
            FocusPane::State => "Add shared header/cookie/variable placeholder",
            FocusPane::Response => "Bind response variable placeholder",
            pane => return self.log(format!("Add placeholder for {}", pane.label())),
        };

        self.log(message);
    }

    fn delete_local_placeholder(&mut self) {
        if self.focus != FocusPane::History {
            self.log(format!("Delete placeholder for {}", self.focus.label()));
            return;
        }

        let Some(row) = self.history_rows().get(self.history_cursor).cloned() else {
            return;
        };

        let deleted = match row {
            HistoryRow::Host { origin, .. } => {
                let deleted = self.history.delete_host(&origin);
                self.expanded_hosts.remove(&origin);
                self.expanded_routes
                    .retain(|key| !key.starts_with(&format!("{origin}\t")));
                if deleted > 0 {
                    self.log(format!("Deleted {deleted} histories for {origin}"));
                }
                deleted
            }
            HistoryRow::Route {
                origin,
                method,
                path,
                display_path,
                ..
            } => {
                let deleted = self.history.delete_route(&origin, &method, &path);
                self.expanded_routes
                    .remove(&route_key(&origin, &method, &path));
                if deleted > 0 {
                    self.log(format!(
                        "Deleted {deleted} histories for {method} {display_path}"
                    ));
                }
                deleted
            }
            HistoryRow::Variant { id, label, .. } => {
                let deleted = usize::from(self.history.delete_entry(&id));
                if deleted > 0 {
                    self.log(format!("Deleted {label}"));
                }
                deleted
            }
            HistoryRow::Empty => 0,
        };

        if deleted == 0 {
            self.log("No history selected to delete");
            return;
        }

        self.refresh_after_history_change();
        self.save_history();
    }

    fn rename_local_placeholder(&mut self) {
        if self.focus != FocusPane::History {
            self.log(format!("Rename placeholder for {}", self.focus.label()));
            return;
        }

        let Some(row) = self.history_rows().get(self.history_cursor).cloned() else {
            return;
        };

        let HistoryRow::Variant { id, label, .. } = row else {
            self.log("Rename applies to request histories for now");
            return;
        };

        self.rename_target_id = Some(id);
        self.rename_input = label;
        self.overlay = Some(Overlay::RenameHistory);
        self.log("Rename history opened");
    }

    fn select_method(&mut self, method: &'static str) {
        self.request.method = method.to_string();
        self.overlay = None;
        self.context_menu = None;
        self.log(format!("Method set to {method}"));
    }

    fn open_body_mode_selector(&mut self) {
        self.context_menu = None;
        self.focus = FocusPane::Body;
        self.overlay = Some(Overlay::BodyModeMenu);
        self.log("Body mode menu opened");
    }

    fn select_body_mode(&mut self, mode: BodyMode) {
        self.request.set_body_mode(mode);
        self.body_fields = body_fields_from_input(mode, &self.body_input);
        if mode.is_key_value_body() {
            self.sync_body_input_from_fields();
        }
        self.overlay = None;
        self.context_menu = None;
        self.log(format!("Body mode set to {}", mode.label()));
    }

    fn submit_history_rename(&mut self) {
        let Some(id) = self.rename_target_id.clone() else {
            self.dispatch(Action::CloseOverlay);
            return;
        };
        let name = self.rename_input.trim();
        let name = (!name.is_empty()).then(|| name.to_string());

        if self.history.rename_entry(&id, name) {
            self.save_history();
            self.log("Renamed history");
        } else {
            self.log("History rename target missing");
        }

        self.overlay = None;
        self.rename_target_id = None;
        self.rename_input.clear();
    }

    fn log(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = message.clone();
        self.logs.push(message);

        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }
}

fn editor_inputs_from_request(request: &RequestDraft) -> (String, String, String, String) {
    (
        format!("{}{}", request.origin, request.path),
        request.query.clone().unwrap_or_default(),
        headers_input_from_request(request),
        request.body.clone(),
    )
}

fn headers_input_from_request(request: &RequestDraft) -> String {
    request
        .headers
        .iter()
        .map(|header| format!("{}: {}", header.name, header.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_headers_input(input: &str) -> Vec<Header> {
    input
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            let name = name.trim();

            if name.is_empty() {
                return None;
            }

            Some(Header::new(name, value.trim()))
        })
        .collect()
}

fn clean_empty_headers(headers: &mut Vec<Header>) {
    headers.retain(|header| !header.name.trim().is_empty() || !header.value.trim().is_empty());
}

fn ensure_header_row(headers: &mut Vec<Header>, index: usize) {
    while headers.len() <= index {
        headers.push(Header::new("", ""));
    }
}

fn push_header_cell(header: &mut Header, column: KeyValueColumn, character: char) {
    match column {
        KeyValueColumn::Key => header.name.push(character),
        KeyValueColumn::Value => header.value.push(character),
    }
}

fn pop_header_cell(header: &mut Header, column: KeyValueColumn) {
    match column {
        KeyValueColumn::Key => {
            header.name.pop();
        }
        KeyValueColumn::Value => {
            header.value.pop();
        }
    }
}

fn ensure_body_field_row(fields: &mut Vec<BodyField>, index: usize) {
    while fields.len() <= index {
        fields.push(BodyField {
            key: String::new(),
            value: String::new(),
        });
    }
}

fn push_body_field_cell(field: &mut BodyField, column: KeyValueColumn, character: char) {
    match column {
        KeyValueColumn::Key => field.key.push(character),
        KeyValueColumn::Value => field.value.push(character),
    }
}

fn pop_body_field_cell(field: &mut BodyField, column: KeyValueColumn) {
    match column {
        KeyValueColumn::Key => {
            field.key.pop();
        }
        KeyValueColumn::Value => {
            field.value.pop();
        }
    }
}

fn body_fields_from_input(mode: BodyMode, input: &str) -> Vec<BodyField> {
    if !mode.is_key_value_body() || input.is_empty() {
        return Vec::new();
    }

    let parts = if input.contains('\n') {
        input.lines().collect::<Vec<_>>()
    } else {
        input.split('&').collect::<Vec<_>>()
    };

    parts
        .into_iter()
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }

            let (key, value) = part.split_once('=')?;
            let key = key.trim();

            if key.is_empty() {
                return None;
            }

            Some(BodyField {
                key: key.to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

fn body_input_from_fields(mode: BodyMode, fields: &[BodyField]) -> String {
    let separator = match mode {
        BodyMode::UrlEncoded => "&",
        BodyMode::FormData => "\n",
        BodyMode::Raw | BodyMode::Binary => "",
    };

    fields
        .iter()
        .filter(|field| !field.key.trim().is_empty() || !field.value.trim().is_empty())
        .map(|field| format!("{}={}", field.key.trim(), field.value.trim()))
        .collect::<Vec<_>>()
        .join(separator)
}

fn is_text_editor(focus: FocusPane) -> bool {
    matches!(
        focus,
        FocusPane::Url | FocusPane::Query | FocusPane::Headers | FocusPane::Body
    )
}

fn accepts_newline(focus: FocusPane) -> bool {
    matches!(focus, FocusPane::Headers | FocusPane::Body)
}

fn route_key(origin: &str, method: &str, path: &str) -> String {
    format!("{origin}\t{method}\t{path}")
}

#[cfg(not(test))]
fn execute_request(request: &RequestDraft, state: &ProjectState) -> Result<HttpResponse, String> {
    crate::net::http::send(request, state)
}

#[cfg(test)]
fn execute_request(_request: &RequestDraft, _state: &ProjectState) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: Vec::new(),
        body: "{}".to_string(),
        truncated: false,
    })
}

impl FocusPane {
    pub fn label(self) -> &'static str {
        match self {
            FocusPane::History => "History",
            FocusPane::Method => "Method",
            FocusPane::Url => "URL",
            FocusPane::Query => "Query",
            FocusPane::Headers => "Headers",
            FocusPane::State => "State",
            FocusPane::Body => "Body",
            FocusPane::Response => "Response",
            FocusPane::Logs => "Logs",
        }
    }
}

impl KeyValueColumn {
    fn label(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Value => "value",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_text(app: &mut App, text: &str) {
        for character in text.chars() {
            let code = if character == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(character)
            };
            app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
        }
    }

    fn poll_until_request_finishes(app: &mut App) {
        for _ in 0..1000 {
            app.poll_request_runner();

            if app.active_run().is_none() {
                return;
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        app.poll_request_runner();
    }

    #[test]
    fn q_does_not_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        assert!(!app.should_quit());
    }

    #[test]
    fn escape_does_not_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(!app.should_quit());
    }

    #[test]
    fn ctrl_c_does_not_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

        assert!(!app.should_quit());
    }

    #[test]
    fn ctrl_q_does_not_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));

        assert!(!app.should_quit());
    }

    #[test]
    fn tab_uses_shared_focus_intent() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

        assert_eq!(app.focus(), FocusPane::Method);
    }

    #[test]
    fn number_keys_are_not_method_shortcuts() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.request.method, "GET");

        app.set_focus(FocusPane::Method);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        assert_eq!(app.request.method, "GET");
    }

    #[test]
    fn question_mark_opens_help() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        assert_eq!(app.overlay(), Some(Overlay::Help));
    }

    #[test]
    fn f5_runs_current_request() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE));

        assert!(app.active_run().is_some());
    }

    #[test]
    fn request_text_panes_accept_a_and_d_as_input() {
        let mut app = App::new();
        app.query_input.clear();
        app.request.set_query("");
        app.set_focus(FocusPane::Query);

        type_text(&mut app, "ad");

        assert_eq!(app.query_input(), "ad");
        assert_eq!(app.request.query.as_deref(), Some("ad"));
    }

    #[test]
    fn run_persists_current_request_to_history() {
        let mut app = App::new();
        app.query_input.clear();
        app.request.set_query("");
        app.set_focus(FocusPane::Query);
        type_text(&mut app, "q=rust");

        app.activate_header_action(HeaderAction::Run);

        assert_eq!(app.history.entries.len(), 1);
        assert_eq!(app.history.entries[0].query.as_deref(), Some("q=rust"));
        assert!(app.active_run().is_some());

        poll_until_request_finishes(&mut app);

        assert_eq!(app.response().map(|response| response.status), Some(200));
        assert!(app.logs().iter().any(
            |log| log.starts_with("GET https://api.example.com") && log.ends_with("-> 200 OK")
        ));
    }

    #[test]
    fn host_path_text_input_updates_request_url() {
        let mut app = App::new();
        app.url_input.clear();
        app.set_focus(FocusPane::Url);

        type_text(&mut app, "https://google.com/search");

        assert_eq!(app.url_input(), "https://google.com/search");
        assert_eq!(app.request.origin, "https://google.com");
        assert_eq!(app.request.path, "/search");
    }

    #[test]
    fn headers_and_body_accept_text_and_newlines() {
        let mut app = App::new();
        app.headers_input.clear();
        app.set_focus(FocusPane::Headers);

        type_text(&mut app, "X-Test: yes\nAccept: application/json");

        assert_eq!(app.headers_input(), "X-Test: yes\nAccept: application/json");
        assert_eq!(app.request.headers.len(), 2);
        assert_eq!(app.request.headers[0], Header::new("X-Test", "yes"));

        app.body_input.clear();
        app.set_focus(FocusPane::Body);
        type_text(&mut app, "{\"hello\":true}\nsecond line");

        assert_eq!(app.body_input(), "{\"hello\":true}\nsecond line");
        assert_eq!(app.request.body, "{\"hello\":true}\nsecond line");
    }

    #[test]
    fn local_headers_support_key_value_cell_editing() {
        let mut app = App::new();
        app.request.headers.clear();
        app.headers_input.clear();

        app.select_local_header_cell(0, KeyValueColumn::Key);
        type_text(&mut app, "Authorization");
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        type_text(&mut app, "Bearer {{access_token}}");

        assert_eq!(
            app.request.headers,
            vec![Header::new("Authorization", "Bearer {{access_token}}")]
        );
        assert_eq!(
            app.headers_input(),
            "Authorization: Bearer {{access_token}}"
        );
    }

    #[test]
    fn shared_headers_are_editable_as_global_key_value_rows() {
        let mut app = App::new();

        app.select_shared_header_cell(0, KeyValueColumn::Key);
        type_text(&mut app, "Authorization");
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        type_text(&mut app, "Bearer {{access_token}}");

        assert_eq!(
            app.state.shared_headers,
            vec![Header::new("Authorization", "Bearer {{access_token}}")]
        );
    }

    #[test]
    fn urlencoded_body_accepts_raw_paste_and_parses_rows() {
        let mut app = App::new();
        app.select_body_mode_option(BodyMode::UrlEncoded);
        app.body_input.clear();
        app.set_focus(FocusPane::Body);

        type_text(&mut app, "q=rust&page=2");

        assert_eq!(app.body_input(), "q=rust&page=2");
        assert_eq!(
            app.body_fields(),
            &[
                BodyField {
                    key: "q".to_string(),
                    value: "rust".to_string()
                },
                BodyField {
                    key: "page".to_string(),
                    value: "2".to_string()
                }
            ]
        );
        assert_eq!(app.request.body, "q=rust&page=2");
    }

    #[test]
    fn key_value_body_does_not_split_json_on_colons() {
        let fields = body_fields_from_input(
            BodyMode::FormData,
            "{\"name\":\"Ada Lovelace\",\"role\":\"admin\",\"active\":true}",
        );

        assert!(fields.is_empty());
    }

    #[test]
    fn urlencoded_body_supports_key_value_cell_editing() {
        let mut app = App::new();
        app.select_body_mode_option(BodyMode::UrlEncoded);
        app.body_input.clear();
        app.body_fields.clear();

        app.select_body_field_cell(0, KeyValueColumn::Key);
        type_text(&mut app, "q");
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        type_text(&mut app, "rust");

        assert_eq!(app.body_input(), "q=rust");
        assert_eq!(app.request.body, "q=rust");
    }

    #[test]
    fn context_menu_adds_and_deletes_local_headers() {
        let mut app = App::new();
        app.request.headers = vec![Header::new("Accept", "application/json")];
        app.sync_headers_input_from_request();

        app.open_context_menu(ContextTarget::LocalHeader(0), 10, 10);
        assert_eq!(app.overlay(), Some(Overlay::ContextMenu));
        assert_eq!(
            app.context_menu_items(),
            vec![
                "Add header below".to_string(),
                "Delete header".to_string(),
                "Clear header".to_string()
            ]
        );

        app.activate_context_menu_row(0);
        assert_eq!(app.request.headers.len(), 2);
        assert_eq!(
            app.active_local_header_cell(),
            Some((1, KeyValueColumn::Key))
        );

        app.request.headers[1] = Header::new("X-Test", "yes");
        app.open_context_menu(ContextTarget::LocalHeader(1), 10, 10);
        app.activate_context_menu_row(1);

        assert_eq!(
            app.request.headers,
            vec![Header::new("Accept", "application/json")]
        );
        assert_eq!(app.headers_input(), "Accept: application/json");
    }

    #[test]
    fn context_menu_controls_body_fields_and_raw_body() {
        let mut app = App::new();
        app.select_body_mode_option(BodyMode::UrlEncoded);
        app.body_fields = vec![
            BodyField {
                key: "q".to_string(),
                value: "rust".to_string(),
            },
            BodyField {
                key: "page".to_string(),
                value: "2".to_string(),
            },
        ];
        app.sync_body_input_from_fields();

        app.open_context_menu(ContextTarget::BodyField(0), 10, 10);
        app.activate_context_menu_row(1);

        assert_eq!(
            app.body_fields(),
            &[BodyField {
                key: "page".to_string(),
                value: "2".to_string()
            }]
        );
        assert_eq!(app.body_input(), "page=2");

        app.select_body_mode_option(BodyMode::Raw);
        app.body_input = "{\"q\":\"rust\"}".to_string();
        app.request.body.clone_from(&app.body_input);
        app.open_context_menu(ContextTarget::BodyRaw, 10, 10);
        app.activate_context_menu_row(0);

        assert!(app.body_input().is_empty());
        assert!(app.request.body.is_empty());
    }

    #[test]
    fn context_menu_can_open_method_dropdown() {
        let mut app = App::new();

        app.open_context_menu(ContextTarget::Method, 10, 10);
        assert_eq!(app.context_menu_items(), vec!["Select method".to_string()]);
        app.activate_context_menu_row(0);

        assert_eq!(app.overlay(), Some(Overlay::MethodMenu));
    }

    #[test]
    fn response_headers_can_be_expanded_and_collapsed() {
        let mut app = App::new();
        app.set_response_for_test(HttpResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: (0..10)
                .map(|index| Header::new(format!("X-Test-{index}"), "yes"))
                .collect(),
            body: String::new(),
            truncated: false,
        });

        assert!(!app.response_headers_expanded());

        app.toggle_response_headers();
        assert!(app.response_headers_expanded());
        assert_eq!(app.focus(), FocusPane::Response);

        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(!app.response_headers_expanded());
    }

    #[test]
    fn method_enter_opens_dropdown_and_selection_closes_it() {
        let mut app = App::new();

        app.set_focus(FocusPane::Method);
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.overlay(), Some(Overlay::MethodMenu));

        app.select_method_option("PATCH");

        assert_eq!(app.request.method, "PATCH");
        assert_eq!(app.overlay(), None);
    }

    #[test]
    fn body_mode_dropdown_selection_updates_request() {
        let mut app = App::new();

        app.open_body_mode_menu();
        assert_eq!(app.overlay(), Some(Overlay::BodyModeMenu));

        app.select_body_mode_option(BodyMode::UrlEncoded);

        assert_eq!(app.body_mode(), BodyMode::UrlEncoded);
        assert_eq!(app.overlay(), None);
    }

    #[test]
    fn help_menu_opens_and_escape_closes_overlay() {
        let mut app = App::new();

        app.activate_header_action(HeaderAction::Help);
        assert_eq!(app.overlay(), Some(Overlay::Help));

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.overlay(), None);
        assert!(!app.should_quit());
    }

    #[test]
    fn curler_menu_opens_about_overlay() {
        let mut app = App::new();

        app.activate_header_action(HeaderAction::Curler);

        assert_eq!(app.overlay(), Some(Overlay::About));
    }

    #[test]
    fn history_clicks_expand_and_select_variant() {
        let mut app = App::new();
        let request =
            RequestDraft::from_curl_args(&["https://google.com/search?q=rust".to_string()])
                .expect("request parses");
        let id = app.history.upsert(request.clone());

        assert_eq!(app.history_rows().len(), 1);

        app.activate_history_row(0);
        assert!(matches!(
            app.history_rows().get(1),
            Some(HistoryRow::Route { .. })
        ));

        app.activate_history_row(1);
        assert!(matches!(
            app.history_rows().get(2),
            Some(HistoryRow::Variant { .. })
        ));

        app.activate_history_row(2);

        assert_eq!(app.selected_history_id.as_deref(), Some(id.as_str()));
        assert_eq!(app.request.url, request.url);
    }

    #[test]
    fn delete_removes_selected_history_variant() {
        let mut app = App::new();
        let request =
            RequestDraft::from_curl_args(&["https://google.com/search?q=rust".to_string()])
                .expect("request parses");
        app.history.upsert(request);

        app.activate_history_row(0);
        app.activate_history_row(1);
        app.activate_history_row(2);
        app.delete_local_placeholder();

        assert!(app.history.entries.is_empty());
        assert_eq!(app.history_rows(), vec![HistoryRow::Empty]);
    }

    #[test]
    fn delete_removes_selected_history_host_subtree() {
        let mut app = App::new();
        app.history.upsert(
            RequestDraft::from_curl_args(&["https://google.com/search?q=rust".to_string()])
                .expect("request parses"),
        );
        app.history.upsert(
            RequestDraft::from_curl_args(&["https://google.com/maps".to_string()])
                .expect("request parses"),
        );

        app.delete_local_placeholder();

        assert!(app.history.entries.is_empty());
        assert_eq!(app.history_rows(), vec![HistoryRow::Empty]);
    }

    #[test]
    fn rename_history_variant_overrides_generated_label() {
        let mut app = App::new();
        let id = app.history.upsert(
            RequestDraft::from_curl_args(&["https://google.com/search?q=rust".to_string()])
                .expect("request parses"),
        );

        app.activate_history_row(0);
        app.activate_history_row(1);
        app.activate_history_row(2);
        app.rename_local_placeholder();
        app.rename_input.clear();
        type_text(&mut app, "search q rust");
        app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            app.history.entries[0].name.as_deref(),
            Some("search q rust")
        );
        assert_eq!(app.overlay(), None);
        assert!(app.history_rows().iter().any(|row| {
            matches!(
                row,
                HistoryRow::Variant { id: row_id, label, .. }
                    if row_id == &id && label == "search q rust"
            )
        }));
    }
}
