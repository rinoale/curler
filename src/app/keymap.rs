use crossterm::event::{KeyEvent, KeyModifiers};
use rustui::keymap::{KeyBinding as FrameworkKeyBinding, Keymap as FrameworkKeymap, binding};

pub(super) use rustui::keymap::{Key, text_input_modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Intent {
    EnterCommandMode,
    Cancel,
    Help,
    ToggleSafeMode,
    RefreshMetadata,
    ToggleFocus,
    Submit,
    Previous,
    Next,
}

pub(super) type KeyBinding = FrameworkKeyBinding<Intent>;

#[derive(Debug, Clone)]
pub(super) struct Keymap {
    inner: FrameworkKeymap<Intent>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::curler()
    }
}

impl Keymap {
    fn curler() -> Self {
        let none = KeyModifiers::NONE;

        Self {
            inner: FrameworkKeymap::new(vec![
                binding(
                    Key::Char(':'),
                    none,
                    Intent::EnterCommandMode,
                    ":",
                    "command mode",
                ),
                binding(Key::Char('?'), none, Intent::Help, "?", "help"),
                binding(Key::Esc, none, Intent::Cancel, "Esc", "cancel"),
                binding(Key::F(2), none, Intent::ToggleSafeMode, "F2", "mode"),
                binding(Key::F(5), none, Intent::RefreshMetadata, "F5", "refresh"),
                binding(Key::Tab, none, Intent::ToggleFocus, "Tab", "focus"),
                binding(
                    Key::BackTab,
                    none,
                    Intent::ToggleFocus,
                    "Shift-Tab",
                    "focus",
                ),
                binding(Key::Enter, none, Intent::Submit, "Enter", "run/open"),
                binding(Key::Up, none, Intent::Previous, "Up", "previous"),
                binding(Key::Down, none, Intent::Next, "Down", "next"),
            ]),
        }
    }

    pub(super) fn intent_for(&self, key: KeyEvent) -> Option<Intent> {
        self.inner.intent_for(key)
    }

    pub(super) fn bindings(&self) -> &[KeyBinding] {
        self.inner.bindings()
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{Intent, Keymap};

    #[test]
    fn quit_is_not_bound_to_a_key() {
        let keymap = Keymap::curler();

        assert_eq!(
            keymap.intent_for(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            keymap.intent_for(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            None
        );
        assert_eq!(
            keymap.intent_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[test]
    fn colon_enters_command_mode() {
        let keymap = Keymap::curler();

        assert_eq!(
            keymap.intent_for(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE)),
            Some(Intent::EnterCommandMode)
        );
    }

    #[test]
    fn escape_cancels_without_quitting() {
        let keymap = Keymap::curler();

        assert_eq!(
            keymap.intent_for(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Intent::Cancel)
        );
    }

    #[test]
    fn question_mark_opens_help() {
        let keymap = Keymap::curler();

        assert_eq!(
            keymap.intent_for(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
            Some(Intent::Help)
        );
    }

    #[test]
    fn exposes_shared_bindings() {
        let keymap = Keymap::curler();
        let labels = keymap
            .bindings()
            .iter()
            .map(|binding| binding.label)
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                ":",
                "?",
                "Esc",
                "F2",
                "F5",
                "Tab",
                "Shift-Tab",
                "Enter",
                "Up",
                "Down"
            ]
        );
    }
}
