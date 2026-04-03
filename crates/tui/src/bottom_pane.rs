#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Chat,
    Command,
    Search,
}

impl InputMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Command => "command",
            Self::Search => "search",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BottomPaneState {
    pub command_menu_index: usize,
}

impl BottomPaneState {
    pub fn input_mode(&self, draft: &str) -> InputMode {
        if draft.trim_start().starts_with('/') {
            InputMode::Command
        } else {
            InputMode::Chat
        }
    }

    pub fn reset_command_selection(&mut self) {
        self.command_menu_index = 0;
    }

    pub fn select_next_command_match(&mut self, total: usize) {
        if total == 0 {
            self.command_menu_index = 0;
            return;
        }

        self.command_menu_index = (self.command_menu_index + 1) % total;
    }

    pub fn select_previous_command_match(&mut self, total: usize) {
        if total == 0 {
            self.command_menu_index = 0;
            return;
        }

        self.command_menu_index = if self.command_menu_index == 0 {
            total - 1
        } else {
            self.command_menu_index - 1
        };
    }
}
