use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    pub email: String,
    pub full_name: Option<String>,
}
