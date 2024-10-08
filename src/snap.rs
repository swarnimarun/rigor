use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub outputs: Vec<Output>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Output {
    pub status_code: u16,
    pub body: serde_json::Value,
}
