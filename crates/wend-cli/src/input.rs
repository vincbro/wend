use crossterm::event::{self, Event, KeyEvent, KeyEventKind, KeyModifiers};
use wend_core::{
    frame::Area,
    input::{
        Input,
        key::{Key, KeyCode, Modifiers},
    },
};

/// Turns a terminal event into an input for the editor, none for events it has no use for.
pub(crate) fn to_input(event: &Event) -> Option<Input> {
    match *event {
        Event::Resize(cols, rows) => Some(Input::Resize(Area::new(cols, rows))),
        Event::Key(key_event) if key_event.kind != KeyEventKind::Release => {
            to_key(key_event).map(Input::Key)
        }
        _ => None,
    }
}

fn to_key(key_event: KeyEvent) -> Option<Key> {
    let code = match key_event.code {
        event::KeyCode::Backspace => KeyCode::Backspace,
        event::KeyCode::Enter => KeyCode::Enter,
        event::KeyCode::Esc => KeyCode::Esc,
        event::KeyCode::Tab | event::KeyCode::BackTab => KeyCode::Tab,
        event::KeyCode::Char(c) => KeyCode::Char(c),
        _ => return None,
    };
    let is_shifted = key_event.modifiers.contains(KeyModifiers::SHIFT)
        || key_event.code == event::KeyCode::BackTab;
    let modifiers = Modifiers {
        alt: key_event.modifiers.contains(KeyModifiers::ALT),
        control: key_event.modifiers.contains(KeyModifiers::CONTROL),
        // A character is already the shifted one, and the keymap binds it that way
        shift: is_shifted && !matches!(code, KeyCode::Char(_)),
    };
    Some(Key { code, modifiers })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(code: event::KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn shifted_characters_drop_shift() {
        let key = to_key(key_event(event::KeyCode::Char('H'), KeyModifiers::SHIFT)).unwrap();
        assert_eq!(
            key,
            Key::new(KeyCode::Char('H')),
            "the keymap binds H, not shift and H"
        );
    }

    #[test]
    fn back_tab_is_shift_tab() {
        let key = to_key(key_event(event::KeyCode::BackTab, KeyModifiers::SHIFT)).unwrap();
        assert_eq!(key.code, KeyCode::Tab);
        assert!(
            key.modifiers.shift,
            "shift stays on keys that aren't characters"
        );
    }

    #[test]
    fn control_stays_on_characters() {
        let key = to_key(key_event(event::KeyCode::Char('s'), KeyModifiers::CONTROL)).unwrap();
        assert!(key.modifiers.control);
    }

    #[test]
    fn resize_becomes_an_input() {
        assert!(matches!(
            to_input(&Event::Resize(80, 24)),
            Some(Input::Resize(Area { cols: 80, rows: 24 }))
        ));
    }

    #[test]
    fn key_releases_are_dropped() {
        let mut release = key_event(event::KeyCode::Char('j'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(
            to_input(&Event::Key(release)).is_none(),
            "or j would move twice"
        );
    }
}
