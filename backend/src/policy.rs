use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPolicy {
    pub auto_approve_ast_safe: bool,
    pub require_human_for_deletions: bool,
    pub strict_secret_scanning: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            auto_approve_ast_safe: true,
            require_human_for_deletions: true,
            strict_secret_scanning: true,
        }
    }
}
