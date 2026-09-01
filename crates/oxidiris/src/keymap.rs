//! Key bindings, declared once as data.
//!
//! Supports OXD-025. See spec §7.
//!
//! The help popup and the key dispatcher both read [`BINDINGS`], so the documented keys and the
//! working keys cannot drift apart. Phase 3 (OXD-030) turns this table into a per-mode map; the
//! shape is chosen so that change is additive.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Something the reader can ask the application to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Start or stop the stream.
    TogglePlay,
    /// Return to the first token.
    Restart,
    /// Increase speed by a coarse step.
    Faster,
    /// Decrease speed by a coarse step.
    Slower,
    /// Increase speed by a fine step.
    FasterFine,
    /// Decrease speed by a fine step.
    SlowerFine,
    /// Step back through the stream.
    Back,
    /// Step forward through the stream.
    Forward,
    /// Jump to the previous paragraph.
    PrevBlock,
    /// Jump to the next paragraph.
    NextBlock,
    /// Jump to the first token.
    GotoStart,
    /// Jump to the last token.
    GotoEnd,
    /// Show or hide the key reference.
    ToggleHelp,
    /// Leave a popup, or do nothing.
    Escape,
    /// Exit the program.
    Quit,
}

/// One documented binding.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    /// Keys that trigger it, as shown to the reader.
    pub keys: &'static str,
    /// What it does.
    pub description: &'static str,
    /// Section heading in the help popup.
    pub group: &'static str,
    /// The action it maps to.
    pub action: Action,
}

/// Every binding the reader can use, in the order the help popup lists them.
pub const BINDINGS: &[Binding] = &[
    Binding {
        keys: "Space",
        description: "Play / pause",
        group: "Playback",
        action: Action::TogglePlay,
    },
    Binding {
        keys: "R",
        description: "Restart from the beginning",
        group: "Playback",
        action: Action::Restart,
    },
    Binding {
        keys: "K / Up",
        description: "Faster (+25 WPM)",
        group: "Speed",
        action: Action::Faster,
    },
    Binding {
        keys: "J / Down",
        description: "Slower (-25 WPM)",
        group: "Speed",
        action: Action::Slower,
    },
    Binding {
        keys: "+",
        description: "Faster, fine (+5 WPM)",
        group: "Speed",
        action: Action::FasterFine,
    },
    Binding {
        keys: "-",
        description: "Slower, fine (-5 WPM)",
        group: "Speed",
        action: Action::SlowerFine,
    },
    Binding {
        keys: "H / Left",
        description: "Back 5 words",
        group: "Navigation",
        action: Action::Back,
    },
    Binding {
        keys: "L / Right",
        description: "Forward 5 words",
        group: "Navigation",
        action: Action::Forward,
    },
    Binding {
        keys: "[",
        description: "Previous paragraph",
        group: "Navigation",
        action: Action::PrevBlock,
    },
    Binding {
        keys: "]",
        description: "Next paragraph",
        group: "Navigation",
        action: Action::NextBlock,
    },
    Binding {
        keys: "g",
        description: "Go to start",
        group: "Navigation",
        action: Action::GotoStart,
    },
    Binding { keys: "G", description: "Go to end", group: "Navigation", action: Action::GotoEnd },
    Binding {
        keys: "?",
        description: "Toggle this help",
        group: "System",
        action: Action::ToggleHelp,
    },
    Binding { keys: "Esc", description: "Close popup", group: "System", action: Action::Escape },
    Binding { keys: "q", description: "Quit", group: "System", action: Action::Quit },
];

/// Map a key event to an action.
///
/// Returns `None` for keys with no binding, and for key *release* events, which Windows terminals
/// emit alongside presses.
pub fn resolve(key: KeyEvent) -> Option<Action> {
    use crossterm::event::KeyEventKind;
    if key.kind == KeyEventKind::Release {
        return None;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Action::Quit),
            _ => None,
        };
    }
    Some(match key.code {
        KeyCode::Char(' ') => Action::TogglePlay,
        KeyCode::Char('r') | KeyCode::Char('R') => Action::Restart,
        KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Up => Action::Faster,
        KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Down => Action::Slower,
        KeyCode::Char('+') | KeyCode::Char('=') => Action::FasterFine,
        KeyCode::Char('-') | KeyCode::Char('_') => Action::SlowerFine,
        KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Left => Action::Back,
        KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Right => Action::Forward,
        KeyCode::Char('[') => Action::PrevBlock,
        KeyCode::Char(']') => Action::NextBlock,
        KeyCode::Char('g') => Action::GotoStart,
        KeyCode::Char('G') => Action::GotoEnd,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Esc => Action::Escape,
        KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
        _ => return None,
    })
}

/// Keys bound to `action`, formatted for display.
///
/// The status bar hints go through here rather than hard-coding key names, so the table stays the
/// only place a binding is described.
pub fn keys_for(action: Action) -> &'static str {
    BINDINGS.iter().find(|b| b.action == action).map_or("", |b| b.keys)
}

/// Group names in display order.
pub fn groups() -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for b in BINDINGS {
        if !seen.contains(&b.group) {
            seen.push(b.group);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn every_documented_binding_is_reachable_from_a_key() {
        // The help popup must not promise an action the dispatcher cannot produce.
        let reachable: Vec<Action> = [
            KeyCode::Char(' '),
            KeyCode::Char('r'),
            KeyCode::Char('k'),
            KeyCode::Char('j'),
            KeyCode::Char('+'),
            KeyCode::Char('-'),
            KeyCode::Char('h'),
            KeyCode::Char('l'),
            KeyCode::Char('['),
            KeyCode::Char(']'),
            KeyCode::Char('g'),
            KeyCode::Char('G'),
            KeyCode::Char('?'),
            KeyCode::Esc,
            KeyCode::Char('q'),
        ]
        .into_iter()
        .filter_map(|c| resolve(press(c)))
        .collect();

        for binding in BINDINGS {
            assert!(
                reachable.contains(&binding.action),
                "binding {:?} is documented but unreachable",
                binding.keys
            );
        }
    }

    #[test]
    fn arrow_keys_mirror_the_vim_keys() {
        assert_eq!(resolve(press(KeyCode::Up)), resolve(press(KeyCode::Char('k'))));
        assert_eq!(resolve(press(KeyCode::Down)), resolve(press(KeyCode::Char('j'))));
        assert_eq!(resolve(press(KeyCode::Left)), resolve(press(KeyCode::Char('h'))));
        assert_eq!(resolve(press(KeyCode::Right)), resolve(press(KeyCode::Char('l'))));
    }

    /// Windows terminals report presses and releases; acting on both would double every keystroke.
    #[test]
    fn key_releases_are_ignored() {
        let mut ev = press(KeyCode::Char(' '));
        ev.kind = KeyEventKind::Release;
        assert_eq!(resolve(ev), None);
    }

    #[test]
    fn ctrl_c_quits_and_other_control_chords_do_nothing() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(resolve(ctrl_c), Some(Action::Quit));
        let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(resolve(ctrl_x), None);
    }

    #[test]
    fn escape_and_quit_are_separate_actions() {
        // Esc closes a popup; only q leaves the program (spec §7.2).
        assert_eq!(resolve(press(KeyCode::Esc)), Some(Action::Escape));
        assert_eq!(resolve(press(KeyCode::Char('q'))), Some(Action::Quit));
    }

    #[test]
    fn unbound_keys_resolve_to_nothing() {
        assert_eq!(resolve(press(KeyCode::Char('z'))), None);
        assert_eq!(resolve(press(KeyCode::F(5))), None);
    }

    #[test]
    fn keys_for_returns_the_documented_key_names() {
        assert_eq!(keys_for(Action::TogglePlay), "Space");
        assert_eq!(keys_for(Action::ToggleHelp), "?");
        assert_eq!(keys_for(Action::Quit), "q");
    }

    #[test]
    fn groups_are_listed_once_each_in_order() {
        assert_eq!(groups(), vec!["Playback", "Speed", "Navigation", "System"]);
    }
}
