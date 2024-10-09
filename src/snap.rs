use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub outputs: Vec<Output>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Output {
    pub method_str: String,
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    pub status_code: u16,
    pub body: serde_json::Value,
}
