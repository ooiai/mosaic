use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    Quit,
    OpenHelp,
    ToggleTurnDetail,
    ToggleTranscriptOverlay,
    ClearDraftOrCloseOverlay,
    ScrollDown,
    ScrollUp,
    ScrollHome,
    ScrollEnd,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
    CommandNext,
    CommandPrevious,
    CommandComplete,
    SubmitComposer,
    BackspaceDraft,
    InsertChar(char),
    None,
}

pub fn interpret_key_event(
    key: KeyEvent,
    command_menu_active: bool,
    command_menu_should_complete: bool,
    draft_empty: bool,
) -> AppEvent {
    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        return AppEvent::Quit;
    }

    if matches!(key.code, KeyCode::F(1)) || (matches!(key.code, KeyCode::Char('?')) && draft_empty)
    {
        return AppEvent::OpenHelp;
    }

    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        return AppEvent::ToggleTurnDetail;
    }

    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        return AppEvent::ToggleTranscriptOverlay;
    }

    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        return AppEvent::CursorHome;
    }

    if matches!(
        key,
        KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }
    ) {
        return AppEvent::CursorEnd;
    }

    if command_menu_active {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => return AppEvent::CommandNext,
            KeyCode::Up | KeyCode::Char('k') => return AppEvent::CommandPrevious,
            KeyCode::Tab if command_menu_should_complete => return AppEvent::CommandComplete,
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => AppEvent::ClearDraftOrCloseOverlay,
        KeyCode::PageDown => AppEvent::ScrollDown,
        KeyCode::PageUp => AppEvent::ScrollUp,
        KeyCode::Left => AppEvent::CursorLeft,
        KeyCode::Right => AppEvent::CursorRight,
        KeyCode::Home => AppEvent::CursorHome,
        KeyCode::End => AppEvent::CursorEnd,
        KeyCode::Enter if command_menu_active && command_menu_should_complete => {
            AppEvent::CommandComplete
        }
        KeyCode::Enter => AppEvent::SubmitComposer,
        KeyCode::Backspace => AppEvent::BackspaceDraft,
        KeyCode::Char(character)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            AppEvent::InsertChar(character)
        }
        _ => AppEvent::None,
    }
}
