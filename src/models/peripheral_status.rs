use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeripheralStatus {
    /// Raw Input device identity, not keystrokes or mouse activity.
    pub device_id: String,
    pub device_name: String,
    pub device_type: String,
    pub connected: bool,
}
