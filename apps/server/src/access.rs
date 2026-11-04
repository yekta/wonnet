//! What a connected app may do with the user's mail, as they chose on the consent page: which
//! accounts it reaches, a role, and privileges the role doesn't cover.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Reader,
    Organizer,
}

impl Role {
    pub const ALL: [Role; 2] = [Role::Reader, Role::Organizer];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Organizer => "organizer",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
    /// Without it an app sees who sent a message, its subject and its snippet, not its body.
    ReadBodies,
    /// Leaving a list reaches the list's sender, outside the mailbox.
    Unsubscribe,
}

impl Privilege {
    pub const ALL: [Privilege; 2] = [Privilege::ReadBodies, Privilege::Unsubscribe];

    pub fn as_str(self) -> &'static str {
        match self {
            Privilege::ReadBodies => "read_bodies",
            Privilege::Unsubscribe => "unsubscribe",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|privilege| privilege.as_str() == text)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Access {
    pub role: Role,
    /// Every account of the user's, the ones added later too. Otherwise only `accounts`.
    pub all_accounts: bool,
    #[serde(default)]
    pub accounts: Vec<Uuid>,
    #[serde(default)]
    pub privileges: Vec<Privilege>,
}

impl Access {
    pub fn reaches(&self, account_id: Uuid) -> bool {
        self.all_accounts || self.accounts.contains(&account_id)
    }

    pub fn organizes(&self) -> bool {
        self.role == Role::Organizer
    }

    pub fn has(&self, privilege: Privilege) -> bool {
        self.privileges.contains(&privilege)
    }
}

/// Who an MCP request comes from: the user, and the grant their app holds.
#[derive(Debug, Clone)]
pub struct Caller {
    pub user_id: Uuid,
    pub grant_id: Uuid,
    pub client_name: String,
    pub access: Access,
}
