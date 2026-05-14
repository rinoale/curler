use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub struct RequestDraft {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Default for RequestDraft {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            url: "https://api.example.com".to_string(),
            headers: vec![
                ("Accept".to_string(), "application/json".to_string()),
                ("User-Agent".to_string(), "curler/0.1".to_string()),
            ],
            body: String::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct App {
    should_quit: bool,
    request: RequestDraft,
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn request(&self) -> &RequestDraft {
        &self.request
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
            _ => {}
        }
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_requests_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        assert!(app.should_quit());
    }

    #[test]
    fn escape_requests_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.should_quit());
    }

    #[test]
    fn ctrl_c_requests_quit() {
        let mut app = App::new();

        app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

        assert!(app.should_quit());
    }
}
