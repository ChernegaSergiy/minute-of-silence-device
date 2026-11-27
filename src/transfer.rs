use khvylyna_core::{ExportBundle, Settings};
use serde_json::Value;

pub fn parse_settings(bytes: &[u8]) -> Option<Settings> {
    let transfer: Value = serde_json::from_slice(bytes).ok()?;
    transfer.get("sender_name")?.as_str()?;
    let json = transfer.get("json")?.as_str()?;
    let bundle: ExportBundle = serde_json::from_str(json).ok()?;
    let mut settings = bundle.settings;
    settings.migrate();
    settings.validate().ok()?;
    Some(settings)
}
