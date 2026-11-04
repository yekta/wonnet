//! The settings kept on this device only: the theme, the undo delay, the notification's button,
//! the settings' sidebar and whether the mailboxes' sidebar is kept open.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

/// What the first button of a new mail's notification does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAction {
    #[default]
    Archive,
    Trash,
}

impl NotificationAction {
    pub const ALL: [NotificationAction; 2] = [Self::Archive, Self::Trash];

    pub fn name(self) -> &'static str {
        match self {
            Self::Archive => "Archive",
            Self::Trash => "Delete",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSettings {
    pub appearance: Appearance,
    /// How long a message waits after Send, in seconds, to be taken back.
    pub undo_delay: u64,
    pub notification_action: NotificationAction,
    pub settings_sidebar: f32,
    /// The sidebar stays open beside the mail, which makes room for it, instead of over it.
    pub keep_sidebar: bool,
    /// The open mail shows beside the list, where the window is wide enough for both.
    pub reading_pane: bool,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            undo_delay: 5,
            notification_action: NotificationAction::Archive,
            settings_sidebar: crate::theme::size::SIDEBAR_WIDTH,
            keep_sidebar: false,
            reading_pane: false,
            path: None,
        }
    }
}

impl LocalSettings {
    pub const UNDO_DELAYS: [u64; 5] = [0, 5, 10, 20, 30];

    pub fn undo_delay_name(seconds: u64) -> String {
        if seconds == 0 { "Off".into() } else { format!("{seconds} s") }
    }

    pub fn load(path: PathBuf) -> Self {
        let mut settings: Self =
            std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        settings.path = Some(path);
        settings
    }

    pub fn save(&self) {
        let Some(path) = self.path.clone() else { return };
        let Ok(bytes) = serde_json::to_vec_pretty(self) else { return };
        std::thread::spawn(move || {
            let _ = std::fs::write(path, bytes);
        });
    }
}
