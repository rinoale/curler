use crossterm::event::{KeyEvent, KeyModifiers};
use rustui::keymap::{Key, KeyBinding as FrameworkKeyBinding, Keymap as FrameworkKeymap, binding};

use super::{Action, Direction, FocusPane};

pub(super) use rustui::keymap::text_input_modifiers;

#[derive(Debug, Clone)]
pub(super) struct Keymap {
    global: FrameworkKeymap<Action>,
    history: FrameworkKeymap<Action>,
    method: FrameworkKeymap<Action>,
    response: FrameworkKeymap<Action>,
    logs: FrameworkKeymap<Action>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::curler()
    }
}

impl Keymap {
    fn curler() -> Self {
        Self {
            global: FrameworkKeymap::new([
                ctrl('q', Action::Quit, "^Q", "Quit Curler"),
                ctrl('r', Action::RunRequest, "^R", "Run current request"),
                ctrl('s', Action::SaveRequest, "^S", "Save current request"),
                ctrl(
                    'p',
                    Action::OpenCommandPalette,
                    "^P",
                    "Open command palette",
                ),
                ctrl(
                    'h',
                    Action::MoveFocus(Direction::Left),
                    "^H",
                    "Move focus left",
                ),
                ctrl(
                    'j',
                    Action::MoveFocus(Direction::Down),
                    "^J",
                    "Move focus down",
                ),
                ctrl('k', Action::MoveFocus(Direction::Up), "^K", "Move focus up"),
                ctrl(
                    'l',
                    Action::MoveFocus(Direction::Right),
                    "^L",
                    "Move focus right",
                ),
                plain(
                    Key::Tab,
                    Action::MoveFocus(Direction::Right),
                    "Tab",
                    "Next pane",
                ),
                plain(
                    Key::BackTab,
                    Action::MoveFocus(Direction::Left),
                    "Shift-Tab",
                    "Previous pane",
                ),
            ]),
            history: FrameworkKeymap::new([
                plain(
                    Key::Char('k'),
                    Action::HistoryUp,
                    "k",
                    "Move history cursor up",
                ),
                plain(Key::Up, Action::HistoryUp, "Up", "Move history cursor up"),
                plain(
                    Key::Char('j'),
                    Action::HistoryDown,
                    "j",
                    "Move history cursor down",
                ),
                plain(
                    Key::Down,
                    Action::HistoryDown,
                    "Down",
                    "Move history cursor down",
                ),
                plain(
                    Key::Enter,
                    Action::ActivateHistory,
                    "Enter",
                    "Open or select history row",
                ),
                plain(
                    Key::Char(' '),
                    Action::ActivateHistory,
                    "Space",
                    "Open or select history row",
                ),
                plain(Key::Char('a'), Action::AddLocal, "a", "Add local item"),
                plain(
                    Key::Char('d'),
                    Action::DeleteLocal,
                    "d",
                    "Delete local item",
                ),
                plain(
                    Key::Char('r'),
                    Action::RenameLocal,
                    "r",
                    "Rename local item",
                ),
            ]),
            method: FrameworkKeymap::new([
                plain(
                    Key::Enter,
                    Action::OpenMethodMenu,
                    "Enter",
                    "Open method menu",
                ),
                plain(
                    Key::Char(' '),
                    Action::OpenMethodMenu,
                    "Space",
                    "Open method menu",
                ),
                plain(
                    Key::Char('1'),
                    Action::SelectMethod("GET"),
                    "1",
                    "Select GET",
                ),
                plain(
                    Key::Char('g'),
                    Action::SelectMethod("GET"),
                    "g",
                    "Select GET",
                ),
                plain(
                    Key::Char('2'),
                    Action::SelectMethod("POST"),
                    "2",
                    "Select POST",
                ),
                plain(
                    Key::Char('p'),
                    Action::SelectMethod("POST"),
                    "p",
                    "Select POST",
                ),
                plain(
                    Key::Char('3'),
                    Action::SelectMethod("PUT"),
                    "3",
                    "Select PUT",
                ),
                plain(
                    Key::Char('u'),
                    Action::SelectMethod("PUT"),
                    "u",
                    "Select PUT",
                ),
                plain(
                    Key::Char('4'),
                    Action::SelectMethod("PATCH"),
                    "4",
                    "Select PATCH",
                ),
                plain(
                    Key::Char('5'),
                    Action::SelectMethod("DELETE"),
                    "5",
                    "Select DELETE",
                ),
                plain(
                    Key::Char('x'),
                    Action::SelectMethod("DELETE"),
                    "x",
                    "Select DELETE",
                ),
            ]),
            response: FrameworkKeymap::new([
                plain(
                    Key::Enter,
                    Action::ToggleResponseHeaders,
                    "Enter",
                    "Toggle response headers",
                ),
                plain(
                    Key::Char(' '),
                    Action::ToggleResponseHeaders,
                    "Space",
                    "Toggle response headers",
                ),
                plain(
                    Key::Char('h'),
                    Action::ToggleResponseHeaders,
                    "h",
                    "Toggle response headers",
                ),
                plain(Key::Char('v'), Action::AddLocal, "v", "Bind response value"),
                plain(
                    Key::Char('y'),
                    Action::EditLocal,
                    "y",
                    "Copy response value",
                ),
            ]),
            logs: FrameworkKeymap::new([plain(
                Key::Char('c'),
                Action::ClearLogs,
                "c",
                "Clear logs",
            )]),
        }
    }

    pub(super) fn global_action(&self, key: KeyEvent) -> Option<Action> {
        self.global.intent_for(key)
    }

    pub(super) fn local_action(&self, focus: FocusPane, key: KeyEvent) -> Option<Action> {
        match focus {
            FocusPane::History => self.history.intent_for(key),
            FocusPane::Method => self.method.intent_for(key),
            FocusPane::Response => self.response.intent_for(key),
            FocusPane::Logs => self.logs.intent_for(key),
            FocusPane::Url
            | FocusPane::Query
            | FocusPane::Headers
            | FocusPane::State
            | FocusPane::Body => None,
        }
    }
}

fn ctrl(
    character: char,
    intent: Action,
    label: &'static str,
    description: &'static str,
) -> FrameworkKeyBinding<Action> {
    binding(
        Key::Char(character),
        KeyModifiers::CONTROL,
        intent,
        label,
        description,
    )
}

fn plain(
    key: Key,
    intent: Action,
    label: &'static str,
    description: &'static str,
) -> FrameworkKeyBinding<Action> {
    binding(key, KeyModifiers::NONE, intent, label, description)
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    #[test]
    fn bare_q_is_not_a_quit_binding() {
        let keymap = Keymap::curler();

        assert_eq!(
            keymap.global_action(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn ctrl_q_maps_to_quit() {
        let keymap = Keymap::curler();

        assert_eq!(
            keymap.global_action(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
    }

    #[test]
    fn method_shortcuts_are_focus_local() {
        let keymap = Keymap::curler();
        let event = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);

        assert_eq!(keymap.local_action(FocusPane::History, event), None);
        assert_eq!(
            keymap.local_action(FocusPane::Method, event),
            Some(Action::SelectMethod("POST"))
        );
    }
}
