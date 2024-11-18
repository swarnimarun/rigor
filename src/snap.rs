use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub outputs: HashSet<Output>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SnapshotDiff {
    pub diffs: Vec<Diff>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Diff {
    pub name: String,
    pub description: String,
    pub data: String,
    pub cause: String,
}

#[derive(Serialize, Deserialize, Eq, Default)]
pub(crate) struct Output {
    pub name: String,
    pub method_str: String,
    pub endpoint: String,
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_payload: Option<serde_json::Value>,
    pub response_body: Option<serde_json::Value>,
}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl std::hash::Hash for Output {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
