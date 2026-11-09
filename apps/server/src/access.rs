//! Who an MCP request comes from and what it may do. The access itself is the protocol's, since
//! the apps show it and choose it.

pub use mail_protocol::access::{Access, Part, Privilege};
use uuid::Uuid;

/// Who an MCP request comes from: the user, and the grant their app holds.
#[derive(Debug, Clone)]
pub struct Caller {
    pub user_id: Uuid,
    pub grant_id: Uuid,
    pub client_name: String,
    pub access: Access,
}
