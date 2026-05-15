use std::{collections::BTreeSet, io};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    history::ProjectHistory,
    project::ProjectContext,
    request::{BodyMode, Header, RequestDraft},
    state::ProjectState,
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
    expanded_hosts: BTreeSet<String>,
    expanded_routes: BTreeSet<String>,
    selected_history_id: Option<String>,
    history_cursor: usize,
    overlay: Option<Overlay>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    Curler,
    Run,
    File,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Left,
    Down,
    Up,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Quit,
    RunRequest,
    SaveRequest,
    OpenCommandPalette,
    MoveFocus(Direction),
    Focus(FocusPane),
    HistoryUp,
    HistoryDown,
    ActivateHistory,
    AddLocal,
    DeleteLocal,
    RenameLocal,
    EditLocal,
    SelectMethod(&'static str),
    OpenBodyModeMenu,
    SelectBodyMode(BodyMode),
    SubmitRename,
    ClearLogs,
    ContextMenu(FocusPane),
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

impl App {
    #[cfg(test)]
    pub fn new() -> Self {
        let request = RequestDraft::default();
        let (url_input, query_input, headers_input, body_input) =
            editor_inputs_from_request(&request);

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
            expanded_hosts: BTreeSet::new(),
            expanded_routes: BTreeSet::new(),
            selected_history_id: None,
            history_cursor: 0,
            overlay: None,
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
            expanded_hosts: BTreeSet::new(),
            expanded_routes: BTreeSet::new(),
            selected_history_id,
            history_cursor: 0,
            overlay: None,
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

    pub fn headers_input(&self) -> &str {
        &self.headers_input
    }

    pub fn body_input(&self) -> &str {
        &self.body_input
    }

    pub fn body_mode(&self) -> BodyMode {
        self.request.body_mode
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

    pub fn history_cursor(&self) -> usize {
        self.history_cursor
    }

    pub fn set_focus(&mut self, focus: FocusPane) {
        if self.focus != focus {
            self.dispatch(Action::Focus(focus));
        }
    }

    pub fn open_context_menu(&mut self, focus: FocusPane) {
        self.dispatch(Action::ContextMenu(focus));
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

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if self.overlay == Some(Overlay::RenameHistory) && self.handle_rename_key_event(key) {
            return;
        }

        if self.overlay.is_some() && key.code == KeyCode::Esc {
            self.dispatch(Action::CloseOverlay);
            return;
        }

        if let Some(action) = global_action(key) {
            self.dispatch(action);
            return;
        }

        if self.handle_text_key_event(key) {
            return;
        }

        if let Some(action) = self.local_action(key) {
            self.dispatch(action);
        }
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit(),
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
            Action::AddLocal => self.add_local_placeholder(),
            Action::DeleteLocal => self.delete_local_placeholder(),
            Action::RenameLocal => self.rename_local_placeholder(),
            Action::EditLocal => self.edit_local_placeholder(),
            Action::SelectMethod(method) => self.select_method(method),
            Action::OpenBodyModeMenu => self.open_body_mode_selector(),
            Action::SelectBodyMode(mode) => self.select_body_mode(mode),
            Action::SubmitRename => self.submit_history_rename(),
            Action::ClearLogs => {
                self.logs.clear();
                self.log("Logs cleared");
            }
            Action::ContextMenu(focus) => {
                self.focus = focus;
                self.log(format!("Context menu placeholder for {}", focus.label()));
            }
            Action::OpenAbout => {
                self.overlay = Some(Overlay::About);
                self.log("Curler menu opened");
            }
            Action::OpenFileMenu => {
                self.overlay = Some(Overlay::FileMenu);
                self.log("File menu opened");
            }
            Action::OpenMethodMenu => {
                self.focus = FocusPane::Method;
                self.overlay = Some(Overlay::MethodMenu);
                self.log("Method menu opened");
            }
            Action::OpenHelp => {
                self.overlay = Some(Overlay::Help);
                self.log("Help opened");
            }
            Action::CloseOverlay => {
                self.overlay = None;
                self.rename_target_id = None;
                self.rename_input.clear();
                self.log("Overlay closed");
            }
        }
    }

    fn quit(&mut self) {
        self.should_quit = true;
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
        if self.overlay.is_some()
            || !is_text_editor(self.focus)
            || !text_input_modifiers(key.modifiers)
        {
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
                }
                true
            }
            _ => false,
        };

        if handled {
            self.try_sync_request_from_editor();
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

    fn try_sync_request_from_editor(&mut self) {
        let _ = self.sync_request_from_editor();
    }

    fn sync_request_from_editor(&mut self) -> Result<(), String> {
        self.request.set_url(&self.url_input)?;
        self.request.set_query(&self.query_input);
        self.request.headers = parse_headers_input(&self.headers_input);
        self.request.body.clone_from(&self.body_input);

        Ok(())
    }

    fn local_action(&self, key: KeyEvent) -> Option<Action> {
        if !key.modifiers.is_empty() {
            return None;
        }

        match self.focus {
            FocusPane::History => match key.code {
                KeyCode::Char('k') | KeyCode::Up => Some(Action::HistoryUp),
                KeyCode::Char('j') | KeyCode::Down => Some(Action::HistoryDown),
                KeyCode::Enter | KeyCode::Char(' ') => Some(Action::ActivateHistory),
                KeyCode::Char('a') => Some(Action::AddLocal),
                KeyCode::Char('d') => Some(Action::DeleteLocal),
                KeyCode::Char('r') => Some(Action::RenameLocal),
                _ => None,
            },
            FocusPane::Method => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => Some(Action::OpenMethodMenu),
                KeyCode::Char('1') | KeyCode::Char('g') => Some(Action::SelectMethod("GET")),
                KeyCode::Char('2') | KeyCode::Char('p') => Some(Action::SelectMethod("POST")),
                KeyCode::Char('3') | KeyCode::Char('u') => Some(Action::SelectMethod("PUT")),
                KeyCode::Char('4') => Some(Action::SelectMethod("PATCH")),
                KeyCode::Char('5') | KeyCode::Char('x') => Some(Action::SelectMethod("DELETE")),
                _ => None,
            },
            FocusPane::Url
            | FocusPane::Query
            | FocusPane::Headers
            | FocusPane::State
            | FocusPane::Body => None,
            FocusPane::Response => match key.code {
                KeyCode::Char('v') => Some(Action::AddLocal),
                KeyCode::Char('y') => Some(Action::EditLocal),
                _ => None,
            },
            FocusPane::Logs => match key.code {
                KeyCode::Char('c') => Some(Action::ClearLogs),
                _ => None,
            },
        }
    }

    fn move_focus(&mut self, direction: Direction) {
        self.focus = match (self.focus, direction) {
            (FocusPane::History, Direction::Right) => FocusPane::Method,
            (FocusPane::History, Direction::Down) => FocusPane::Logs,
            (FocusPane::Method, Direction::Left) => FocusPane::History,
            (FocusPane::Method, Direction::Right) => FocusPane::Url,
            (FocusPane::Method, Direction::Down) => FocusPane::Headers,
            (FocusPane::Url, Direction::Left) => FocusPane::Method,
            (FocusPane::Url, Direction::Right) => FocusPane::Query,
            (FocusPane::Url, Direction::Down) => FocusPane::Headers,
            (FocusPane::Query, Direction::Left) => FocusPane::Url,
            (FocusPane::Query, Direction::Down) => FocusPane::Headers,
            (FocusPane::Headers, Direction::Left) => FocusPane::History,
            (FocusPane::Headers, Direction::Up) => FocusPane::Method,
            (FocusPane::Headers, Direction::Down) => FocusPane::State,
            (FocusPane::State, Direction::Left) => FocusPane::History,
            (FocusPane::State, Direction::Up) => FocusPane::Headers,
            (FocusPane::State, Direction::Down) => FocusPane::Body,
            (FocusPane::Body, Direction::Left) => FocusPane::History,
            (FocusPane::Body, Direction::Right) => FocusPane::Response,
            (FocusPane::Body, Direction::Up) => FocusPane::State,
            (FocusPane::Body, Direction::Down) => FocusPane::Logs,
            (FocusPane::Response, Direction::Left) => FocusPane::Body,
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
        if self.persist_current_request() {
            self.log("Saved current request");
        }
    }

    fn run_current_request(&mut self) {
        self.overlay = None;
        if !self.persist_current_request() {
            return;
        }

        self.log("Run request placeholder");
    }

    fn persist_current_request(&mut self) -> bool {
        if let Err(error) = self.sync_request_from_editor() {
            self.log(format!("Request edit invalid: {error}"));
            return false;
        }

        let id = self.history.upsert(self.request.clone());
        self.selected_history_id = Some(id);
        self.state.merge_from_request(&self.request);

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

    fn edit_local_placeholder(&mut self) {
        self.log(format!("Edit placeholder for {}", self.focus.label()));
    }

    fn select_method(&mut self, method: &'static str) {
        self.request.method = method.to_string();
        self.overlay = None;
        self.log(format!("Method set to {method}"));
    }

    fn open_body_mode_selector(&mut self) {
        self.focus = FocusPane::Body;
        self.overlay = Some(Overlay::BodyModeMenu);
        self.log("Body mode menu opened");
    }

    fn select_body_mode(&mut self, mode: BodyMode) {
        self.request.set_body_mode(mode);
        self.overlay = None;
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

fn is_text_editor(focus: FocusPane) -> bool {
    matches!(
        focus,
        FocusPane::Url | FocusPane::Query | FocusPane::Headers | FocusPane::Body
    )
}

fn accepts_newline(focus: FocusPane) -> bool {
    matches!(focus, FocusPane::Headers | FocusPane::Body)
}

fn text_input_modifiers(mut modifiers: KeyModifiers) -> bool {
    modifiers.remove(KeyModifiers::SHIFT);
    modifiers.is_empty()
}

fn route_key(origin: &str, method: &str, path: &str) -> String {
    format!("{origin}\t{method}\t{path}")
}

fn global_action(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::RunRequest)
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::SaveRequest)
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::OpenCommandPalette)
        }
        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveFocus(Direction::Left))
        }
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveFocus(Direction::Down))
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveFocus(Direction::Up))
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::MoveFocus(Direction::Right))
        }
        KeyCode::Tab => Some(Action::MoveFocus(Direction::Right)),
        KeyCode::BackTab => Some(Action::MoveFocus(Direction::Left)),
        _ => None,
    }
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
    fn ctrl_q_requests_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL));

        assert!(app.should_quit());
    }

    #[test]
    fn ctrl_l_moves_focus_globally() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));

        assert_eq!(app.focus(), FocusPane::Method);
    }

    #[test]
    fn method_shortcuts_are_local_to_method_pane() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(app.request.method, "GET");

        app.set_focus(FocusPane::Method);
        app.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));

        assert_eq!(app.request.method, "POST");
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
        assert!(
            app.logs()
                .iter()
                .any(|log| log == "Run request placeholder")
        );
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
        app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

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

        app.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

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
        app.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
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
