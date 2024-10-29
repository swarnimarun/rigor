use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub outputs: Vec<Output>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Output {
    pub method_str: String,
    pub endpoint: String,
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_payload: Option<serde_json::Value>,
    pub response_body: Option<serde_json::Value>,
}
