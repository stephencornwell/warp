use std::{collections::HashMap, sync::LazyLock};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::search::slash_command_menu::{static_commands::Argument, StaticCommand};
use crate::ui_components::color_dot;

use super::Availability;

pub static RENAME_TAB: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/rename-tab",
    description: "Rename the current tab",
    icon_path: "bundled/svg/pencil-line.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: Some(Argument::required().with_hint_text("<tab name>")),
});

static SET_TAB_COLOR_HINT: LazyLock<String> = LazyLock::new(|| {
    let mut hint = String::from("<");
    for color in color_dot::TAB_COLOR_OPTIONS {
        hint.push_str(&color.to_string().to_ascii_lowercase());
        hint.push('|');
    }
    hint.push_str("none>");
    hint
});

pub static SET_TAB_COLOR: LazyLock<StaticCommand> = LazyLock::new(|| StaticCommand {
    name: "/set-tab-color",
    description: "Set the color of the current tab",
    icon_path: "bundled/svg/ellipse.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: Some(Argument::required().with_hint_text(SET_TAB_COLOR_HINT.as_str())),
});

pub const OPEN_SETTINGS_FILE: StaticCommand = StaticCommand {
    name: "/open-settings-file",
    description: "Open settings file (TOML)",
    icon_path: "bundled/svg/file-code-02.svg",
    availability: Availability::LOCAL,
    auto_enter_ai_mode: false,
    argument: None,
};

pub const CHANGELOG: StaticCommand = StaticCommand {
    name: "/changelog",
    description: "Open the latest changelog",
    icon_path: "bundled/svg/book-open.svg",
    availability: Availability::ALWAYS,
    auto_enter_ai_mode: false,
    argument: None,
};

/// If `query` starts with the given command `name` followed by a space,
/// returns the remainder of the query. Otherwise returns `None`.
pub fn strip_command_prefix(query: &str, name: &str) -> Option<String> {
    query
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix(' '))
        .map(|rest| rest.to_string())
}

pub static COMMAND_REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// A unique identifier for a static slash command.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SlashCommandId(Uuid);

impl SlashCommandId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SlashCommandId {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Registry {
    commands: HashMap<SlashCommandId, StaticCommand>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        let mut commands = HashMap::new();
        for command in all_commands().into_iter() {
            debug_assert!(
                !command
                    .availability
                    .contains(Availability::TERMINAL_VIEW | Availability::AGENT_VIEW),
                "command `{}` sets both TERMINAL_VIEW and AGENT_VIEW, which is unsatisfiable",
                command.name,
            );
            commands.insert(SlashCommandId::new(), command);
        }
        Self { commands }
    }

    pub fn all_commands_by_id(&self) -> impl Iterator<Item = (SlashCommandId, &StaticCommand)> {
        self.commands.iter().map(|(id, cmd)| (*id, cmd))
    }

    pub fn all_commands(&self) -> impl Iterator<Item = &StaticCommand> {
        self.commands.values()
    }

    pub fn get_command(&self, id: &SlashCommandId) -> Option<&StaticCommand> {
        self.commands.get(id)
    }

    pub fn get_command_with_name(&self, name: &str) -> Option<&StaticCommand> {
        self.commands.values().find(|command| command.name == name)
    }

    #[cfg(test)]
    pub fn get_command_id_with_name(&self, name: &str) -> Option<&SlashCommandId> {
        self.commands
            .iter()
            .find(|(_, command)| command.name == name)
            .map(|(id, _)| id)
    }
}

fn all_commands() -> Vec<StaticCommand> {
    vec![
        RENAME_TAB.clone(),
        SET_TAB_COLOR.clone(),
        CHANGELOG,
        OPEN_SETTINGS_FILE,
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn command_names_are_unique() {
        let names = COMMAND_REGISTRY.all_commands().map(|command| command.name);
        let mut seen = HashSet::new();
        for name in names {
            assert!(seen.insert(name), "duplicate slash command name: {name}");
        }
    }

    #[test]
    fn rename_tab_command_requires_argument() {
        let command = COMMAND_REGISTRY
            .get_command_with_name(RENAME_TAB.name)
            .expect("expected /rename-tab to be registered");
        let argument = command
            .argument
            .as_ref()
            .expect("expected /rename-tab to require an argument");

        assert!(!argument.is_optional);
        assert!(!argument.should_execute_on_selection);
        assert_eq!(argument.hint_text, Some("<tab name>"));
    }

    #[test]
    fn set_tab_color_command_requires_argument() {
        let command = COMMAND_REGISTRY
            .get_command_with_name(SET_TAB_COLOR.name)
            .expect("expected /set-tab-color to be registered");
        let argument = command
            .argument
            .as_ref()
            .expect("expected /set-tab-color to require an argument");

        assert!(!argument.is_optional);
        assert!(!argument.should_execute_on_selection);

        let hint = argument
            .hint_text
            .expect("/set-tab-color hint text is set dynamically");
        for color in color_dot::TAB_COLOR_OPTIONS {
            let lower = color.to_string().to_ascii_lowercase();
            assert!(hint.contains(&lower), "hint should mention `{lower}`");
        }
        assert!(hint.contains("none"), "hint should mention `none`");
    }

    #[test]
    fn strip_command_prefix_matches_orchestrate() {
        let result = strip_command_prefix("/orchestrate deploy services", "/orchestrate");
        assert_eq!(result, Some("deploy services".to_string()));
    }

    #[test]
    fn strip_command_prefix_no_match() {
        let result = strip_command_prefix("just a normal query", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_empty() {
        let result = strip_command_prefix("", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_no_trailing_space() {
        // "/plan" alone (no trailing space) should NOT be stripped
        let result = strip_command_prefix("/plan", "/plan");
        assert_eq!(result, None);
    }

    #[test]
    fn strip_command_prefix_trailing_space_only() {
        // "/plan " with nothing after should strip to empty string
        let result = strip_command_prefix("/plan ", "/plan");
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn strip_command_prefix_substring_not_matched() {
        // "/planning" should not match "/plan"
        let result = strip_command_prefix("/planning something", "/plan");
        assert_eq!(result, None);
    }
}
