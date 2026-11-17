use serde::Serialize;

use super::format::BackupSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreMode {
    Merge,
    Replace,
}
#[derive(Clone, Debug, Serialize)]
pub struct RestoreItem {
    pub index: usize,
    pub issuer: String,
    pub account_name: String,
    pub status: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct RestorePreview {
    pub preview_id: String,
    pub mode: RestoreMode,
    pub account_count: usize,
    pub category_count: usize,
    pub existing_count: usize,
    pub items: Vec<RestoreItem>,
}
impl RestorePreview {
    pub(super) fn from_decoded(
        summary: &BackupSummary,
        mode: RestoreMode,
        accounts: &[(String, String)],
        statuses: Vec<String>,
        existing_count: usize,
    ) -> Self {
        Self {
            preview_id: uuid::Uuid::new_v4().to_string(),
            mode,
            account_count: summary.account_count,
            category_count: summary.category_count,
            existing_count,
            items: statuses
                .into_iter()
                .enumerate()
                .map(|(index, status)| RestoreItem {
                    index,
                    issuer: accounts
                        .get(index)
                        .map(|item| item.0.clone())
                        .unwrap_or_default(),
                    account_name: accounts
                        .get(index)
                        .map(|item| item.1.clone())
                        .unwrap_or_default(),
                    status,
                })
                .collect(),
        }
    }
}
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreSelection {
    pub account_indices: Vec<usize>,
    #[serde(default)]
    pub confirm_replace: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct RestoreResult {
    pub restored_accounts: usize,
    pub restored_categories: usize,
    pub mode: RestoreMode,
}
