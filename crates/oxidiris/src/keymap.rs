//! Modal key bindings, declared once as data.
//!
//! Implements OXD-025 and OXD-030. See spec §7.
//!
//! # Why the map is modal
//!
//! `J`/`K` adjust speed while reading, but once focus moves to the full-text panel they have to
//! scroll it — the same key cannot mean both at once (spec §7.1). So a binding declares which
//! [`Mode`]s it applies in, and [`resolve`] is asked for a key *in a mode*.
//!
//! The help popup and the dispatcher both read [`BINDINGS`], so the documented keys and the
//! working keys cannot drift apart.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Which panel currently owns the keyboard (spec §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Reading. Keys drive playback and speed.
    #[default]
    Reader,
    /// The full-text panel has focus. Keys scroll it.
    Browser,
    /// The outline sidebar has focus. Keys move the selection.
    Outline,
}

impl Mode {
    /// Short label for the status bar.
    pub const fn label(self) -> &'static str {
        match self {
            Mode::Reader => "READER",
            Mode::Browser => "BROWSER",
            Mode::Outline => "OUTLINE",
        }
    }

    /// Whether keys in this mode move a panel rather than the reading cursor.
    pub const fn is_panel(self) -> bool {
        matches!(self, Mode::Browser | Mode::Outline)
    }
}

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
    /// Move focus between the reader and the full-text panel.
    FocusPanel,
    /// Show or hide the outline sidebar.
    ToggleOutline,
    /// Show or hide the paragraph just read.
    ToggleReview,
    /// Scroll the focused panel up, or move the outline selection up.
    ScrollUp,
    /// Scroll the focused panel down, or move the outline selection down.
    ScrollDown,
    /// Act on the focused panel's selection.
    Select,
    /// Show or hide the key reference.
    ToggleHelp,
    /// Leave a popup or a sub-mode, and return to reading.
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
    /// Modes this binding applies in.
    pub modes: &'static [Mode],
}

/// Bindings that work wherever the reader is.
const ANY: &[Mode] = &[Mode::Reader, Mode::Browser, Mode::Outline];
/// Bindings that only make sense while reading.
const READING: &[Mode] = &[Mode::Reader];
/// Bindings that only make sense while a panel has focus.
const PANELS: &[Mode] = &[Mode::Browser, Mode::Outline];
/// Bindings that only make sense in the outline.
const OUTLINE: &[Mode] = &[Mode::Outline];

/// Shorthand so the table below stays one row per binding.
const fn bind(
    keys: &'static str,
    description: &'static str,
    group: &'static str,
    action: Action,
    modes: &'static [Mode],
) -> Binding {
    Binding { keys, description, group, action, modes }
}

/// Every binding the reader can use, in the order the help popup lists them.
pub const BINDINGS: &[Binding] = &[
    bind("Space", "Play / pause", "Playback", Action::TogglePlay, ANY),
    bind("R", "Restart from the beginning", "Playback", Action::Restart, READING),
    bind("K / Up", "Faster (+25 WPM)", "Speed", Action::Faster, READING),
    bind("J / Down", "Slower (-25 WPM)", "Speed", Action::Slower, READING),
    bind("+", "Faster, fine (+5 WPM)", "Speed", Action::FasterFine, READING),
    bind("-", "Slower, fine (-5 WPM)", "Speed", Action::SlowerFine, READING),
    bind("H / Left", "Back 5 words", "Navigation", Action::Back, READING),
    bind("L / Right", "Forward 5 words", "Navigation", Action::Forward, READING),
    bind("[", "Previous paragraph", "Navigation", Action::PrevBlock, READING),
    bind("]", "Next paragraph", "Navigation", Action::NextBlock, READING),
    bind("g", "Go to start", "Navigation", Action::GotoStart, READING),
    bind("G", "Go to end", "Navigation", Action::GotoEnd, READING),
    bind("Tab", "Focus the text panel", "Context", Action::FocusPanel, READING),
    bind("o", "Outline sidebar", "Context", Action::ToggleOutline, ANY),
    bind("v", "Review the paragraph just read", "Context", Action::ToggleReview, READING),
    bind("K / Up", "Scroll up", "Panel", Action::ScrollUp, PANELS),
    bind("J / Down", "Scroll down", "Panel", Action::ScrollDown, PANELS),
    bind("Enter", "Jump to the selected heading", "Panel", Action::Select, OUTLINE),
    bind("Tab", "Back to reading", "Panel", Action::FocusPanel, PANELS),
    bind("?", "Toggle this help", "System", Action::ToggleHelp, ANY),
    bind("Esc", "Close popup, return to reading", "System", Action::Escape, ANY),
    bind("q", "Quit", "System", Action::Quit, ANY),
];

/// Map a key event to an action, interpreted in `mode`.
///
/// Returns `None` for keys with no binding in this mode, and for key *release* events, which
/// Windows terminals emit alongside presses.
pub fn resolve(key: KeyEvent, mode: Mode) -> Option<Action> {
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
    if let Some(action) = resolve_anywhere(key.code) {
        return Some(action);
    }
    match mode {
        Mode::Reader => resolve_reading(key.code),
        Mode::Browser | Mode::Outline => resolve_panel(key.code, mode),
    }
}

/// Keys that mean the same thing in every mode.
fn resolve_anywhere(code: KeyCode) -> Option<Action> {
    Some(match code {
        KeyCode::Char(' ') => Action::TogglePlay,
        KeyCode::Char('o') | KeyCode::Char('O') => Action::ToggleOutline,
        KeyCode::Tab | KeyCode::BackTab => Action::FocusPanel,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Esc => Action::Escape,
        KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
        _ => return None,
    })
}

/// Keys that drive the reading cursor.
fn resolve_reading(code: KeyCode) -> Option<Action> {
    Some(match code {
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
        KeyCode::Char('v') | KeyCode::Char('V') => Action::ToggleReview,
        _ => return None,
    })
}

/// Keys that move a focused panel. `J`/`K` scroll here rather than changing speed (spec §7.3).
fn resolve_panel(code: KeyCode, mode: Mode) -> Option<Action> {
    Some(match code {
        KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Up => Action::ScrollUp,
        KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Down => Action::ScrollDown,
        KeyCode::Enter if mode == Mode::Outline => Action::Select,
        _ => return None,
    })
}

/// Bindings that apply in `mode`, in display order.
pub fn bindings_for(mode: Mode) -> impl Iterator<Item = &'static Binding> {
    BINDINGS.iter().filter(move |b| b.modes.contains(&mode))
}

/// Keys bound to `action`, formatted for display.
///
/// The status bar hints go through here rather than hard-coding key names, so the table stays the
/// only place a binding is described.
pub fn keys_for(action: Action) -> &'static str {
    BINDINGS.iter().find(|b| b.action == action).map_or("", |b| b.keys)
}

/// Group names that have at least one binding in `mode`, in display order.
pub fn groups_for(mode: Mode) -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for b in bindings_for(mode) {
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

    /// Every key that appears anywhere in the table, so coverage tests cannot go stale silently.
    const PROBED: &[KeyCode] = &[
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
        KeyCode::Char('v'),
        KeyCode::Char('o'),
        KeyCode::Tab,
        KeyCode::Enter,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Char('?'),
        KeyCode::Esc,
        KeyCode::Char('q'),
    ];

    #[test]
    fn every_documented_binding_is_reachable_in_the_modes_it_claims() {
        // The help popup must not promise an action the dispatcher cannot produce in that mode.
        for mode in [Mode::Reader, Mode::Browser, Mode::Outline] {
            let reachable: Vec<Action> =
                PROBED.iter().filter_map(|c| resolve(press(*c), mode)).collect();
            for binding in bindings_for(mode) {
                assert!(
                    reachable.contains(&binding.action),
                    "{:?} is documented in {mode:?} but unreachable",
                    binding.keys
                );
            }
        }
    }

    /// The conflict spec §7.1 calls out: the same key, two meanings, decided by mode.
    #[test]
    fn j_and_k_change_speed_while_reading_and_scroll_in_a_panel() {
        assert_eq!(resolve(press(KeyCode::Char('k')), Mode::Reader), Some(Action::Faster));
        assert_eq!(resolve(press(KeyCode::Char('j')), Mode::Reader), Some(Action::Slower));
        assert_eq!(resolve(press(KeyCode::Char('k')), Mode::Browser), Some(Action::ScrollUp));
        assert_eq!(resolve(press(KeyCode::Char('j')), Mode::Browser), Some(Action::ScrollDown));
        assert_eq!(resolve(press(KeyCode::Char('k')), Mode::Outline), Some(Action::ScrollUp));
    }

    #[test]
    fn arrow_keys_mirror_the_vim_keys_in_every_mode() {
        for mode in [Mode::Reader, Mode::Browser, Mode::Outline] {
            assert_eq!(
                resolve(press(KeyCode::Up), mode),
                resolve(press(KeyCode::Char('k')), mode),
                "arrows diverged from hjkl in {mode:?}"
            );
            assert_eq!(
                resolve(press(KeyCode::Down), mode),
                resolve(press(KeyCode::Char('j')), mode)
            );
        }
        assert_eq!(resolve(press(KeyCode::Left), Mode::Reader), Some(Action::Back));
        assert_eq!(resolve(press(KeyCode::Right), Mode::Reader), Some(Action::Forward));
    }

    #[test]
    fn seek_keys_do_nothing_while_a_panel_has_focus() {
        for code in [KeyCode::Char('h'), KeyCode::Char('l'), KeyCode::Char('['), KeyCode::Char('v')]
        {
            assert_eq!(resolve(press(code), Mode::Browser), None, "{code:?} leaked into Browser");
        }
    }

    #[test]
    fn enter_selects_only_in_the_outline() {
        assert_eq!(resolve(press(KeyCode::Enter), Mode::Outline), Some(Action::Select));
        assert_eq!(resolve(press(KeyCode::Enter), Mode::Browser), None);
        assert_eq!(resolve(press(KeyCode::Enter), Mode::Reader), None);
    }

    #[test]
    fn play_pause_and_the_system_keys_work_in_every_mode() {
        for mode in [Mode::Reader, Mode::Browser, Mode::Outline] {
            assert_eq!(resolve(press(KeyCode::Char(' ')), mode), Some(Action::TogglePlay));
            assert_eq!(resolve(press(KeyCode::Char('?')), mode), Some(Action::ToggleHelp));
            assert_eq!(resolve(press(KeyCode::Char('q')), mode), Some(Action::Quit));
            assert_eq!(resolve(press(KeyCode::Tab), mode), Some(Action::FocusPanel));
            assert_eq!(resolve(press(KeyCode::Char('o')), mode), Some(Action::ToggleOutline));
        }
    }

    /// Windows terminals report presses and releases; acting on both would double every keystroke.
    #[test]
    fn key_releases_are_ignored() {
        let mut ev = press(KeyCode::Char(' '));
        ev.kind = KeyEventKind::Release;
        assert_eq!(resolve(ev, Mode::Reader), None);
    }

    #[test]
    fn ctrl_c_quits_and_other_control_chords_do_nothing() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(resolve(ctrl_c, Mode::Reader), Some(Action::Quit));
        let ctrl_x = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(resolve(ctrl_x, Mode::Reader), None);
    }

    #[test]
    fn escape_and_quit_are_separate_actions() {
        // Esc leaves a sub-mode; only q leaves the program (spec §7.2).
        assert_eq!(resolve(press(KeyCode::Esc), Mode::Browser), Some(Action::Escape));
        assert_eq!(resolve(press(KeyCode::Char('q')), Mode::Browser), Some(Action::Quit));
    }

    #[test]
    fn unbound_keys_resolve_to_nothing() {
        assert_eq!(resolve(press(KeyCode::Char('z')), Mode::Reader), None);
        assert_eq!(resolve(press(KeyCode::F(5)), Mode::Outline), None);
    }

    #[test]
    fn keys_for_returns_the_documented_key_names() {
        assert_eq!(keys_for(Action::TogglePlay), "Space");
        assert_eq!(keys_for(Action::ToggleHelp), "?");
        assert_eq!(keys_for(Action::Quit), "q");
    }

    #[test]
    fn groups_are_listed_once_each_in_order_and_vary_by_mode() {
        assert_eq!(
            groups_for(Mode::Reader),
            vec!["Playback", "Speed", "Navigation", "Context", "System"]
        );
        assert_eq!(groups_for(Mode::Browser), vec!["Playback", "Context", "Panel", "System"]);
    }

    #[test]
    fn speed_bindings_are_not_offered_while_a_panel_has_focus() {
        let browser: Vec<Action> = bindings_for(Mode::Browser).map(|b| b.action).collect();
        assert!(!browser.contains(&Action::Faster));
        assert!(browser.contains(&Action::ScrollUp));
    }
}
