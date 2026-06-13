use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::tui::Pane;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    CyclePane,
    CommandMode,
    MoveDown,
    MoveUp,
    GoTop,
    GoBottom,
    PageDown,
    PageUp,
    Select,
    ApprovePlan,
    RejectPlan,
    EditPlan,
    FocusPane(Pane),
    OpenCommandPalette,
    ShowHelp,
    DismissOverlay,
    MouseFocus { column: u16, row: u16 },
}

#[derive(Debug, Default)]
pub struct KeyBindings {
    pending_g: bool,
}

impl KeyBindings {
    pub fn handle_key(&mut self, key: KeyEvent, active_pane: Pane) -> Option<Action> {
        if key.code != KeyCode::Char('g') {
            self.pending_g = false;
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers.is_empty() => Some(Action::Quit),
            KeyCode::Tab => Some(Action::CyclePane),
            KeyCode::Char(':') if key.modifiers.is_empty() => Some(Action::CommandMode),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
            KeyCode::Char('G') => Some(Action::GoBottom),
            KeyCode::Char('g') if self.pending_g => {
                self.pending_g = false;
                Some(Action::GoTop)
            }
            KeyCode::Char('g') => {
                self.pending_g = true;
                None
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::PageDown)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Action::PageUp)
            }
            KeyCode::Enter => Some(Action::Select),
            KeyCode::Esc => Some(Action::DismissOverlay),
            KeyCode::Char('a') if active_pane == Pane::PlanApproval => Some(Action::ApprovePlan),
            KeyCode::Char('r') if active_pane == Pane::PlanApproval => Some(Action::RejectPlan),
            KeyCode::Char('e') if active_pane == Pane::PlanApproval => Some(Action::EditPlan),
            KeyCode::Char('p')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::SUPER) =>
            {
                Some(Action::OpenCommandPalette)
            }
            KeyCode::Char('?') if key.modifiers.is_empty() => Some(Action::ShowHelp),
            KeyCode::F(1) => Some(Action::ShowHelp),
            KeyCode::F(2) => Some(Action::FocusPane(Pane::Editor)),
            KeyCode::F(3) => Some(Action::FocusPane(Pane::Agents)),
            KeyCode::F(4) => Some(Action::FocusPane(Pane::PlanApproval)),
            _ => None,
        }
    }

    pub fn handle_mouse(&self, mouse: MouseEvent) -> Option<Action> {
        match mouse.kind {
            MouseEventKind::Down(_) => Some(Action::MouseFocus {
                column: mouse.column,
                row: mouse.row,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn maps_global_keys_and_function_panes() {
        let mut bindings = KeyBindings::default();

        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('q')), Pane::Editor),
            Some(Action::Quit)
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::Tab), Pane::Editor),
            Some(Action::CyclePane)
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::F(2)), Pane::Files),
            Some(Action::FocusPane(Pane::Editor))
        );
    }

    #[test]
    fn maps_overlay_keys() {
        let mut bindings = KeyBindings::default();

        assert_eq!(
            bindings.handle_key(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
                Pane::Editor
            ),
            Some(Action::OpenCommandPalette)
        );
        assert_eq!(
            bindings.handle_key(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::SUPER),
                Pane::Editor
            ),
            Some(Action::OpenCommandPalette)
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::F(1)), Pane::Editor),
            Some(Action::ShowHelp)
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('q')), Pane::Editor),
            Some(Action::Quit)
        );
    }

    #[test]
    fn maps_vim_navigation_sequences() {
        let mut bindings = KeyBindings::default();

        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('j')), Pane::Files),
            Some(Action::MoveDown)
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('g')), Pane::Files),
            None
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('g')), Pane::Files),
            Some(Action::GoTop)
        );
        assert_eq!(
            bindings.handle_key(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                Pane::Editor
            ),
            Some(Action::PageDown)
        );
    }

    #[test]
    fn plan_actions_are_pane_scoped() {
        let mut bindings = KeyBindings::default();

        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('a')), Pane::Editor),
            None
        );
        assert_eq!(
            bindings.handle_key(key(KeyCode::Char('a')), Pane::PlanApproval),
            Some(Action::ApprovePlan)
        );
    }
}
