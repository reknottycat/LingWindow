use serde::Serialize;
use tauri::AppHandle;

use crate::i18n::{tr, Key};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current_version: String,
    available: bool,
    version: Option<String>,
    notes: Option<String>,
}

// Never contact the upstream updater, even when an old preference enables it.
pub async fn check(_app: &AppHandle) -> Result<UpdateCheck, String> {
    Err(tr(Key::UpdateDistributionUnavailable).into())
}

pub async fn install(_app: &AppHandle) -> Result<String, String> {
    Err(tr(Key::UpdateDistributionUnavailable).into())
}
