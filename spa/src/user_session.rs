use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct UserSession {
    pub user_id: Uuid,
    pub email: String,
    pub token: String,
}

impl UserSession {
    pub fn logged(&self) -> bool {
        !self.token.is_empty()
    }
}
