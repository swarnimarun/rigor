use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr, path::PathBuf,
};

use serde_json::{json, Value};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Rigor {
    /// multiple test cases can contain the same endpoint
    pub tests: Vec<TestCase>,
    /// we only support running against a single endpoint per rigor file
    pub endpoint: String,
    /// if true, all values of the format ${ENV_VAR} will be replaced by respective env variable
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub use_env: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestCase {
    pub name: String,
    pub route: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_payload_fields: Option<Vec<String>>,
}

impl Rigor {
    pub(crate) fn get_path(path: Option<PathBuf>) -> PathBuf {
        path.unwrap_or_else(|| {
            std::env::var("RIGOR_PATH")
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    std::env::current_dir()
                        .expect("failed to get current directory")
                        .join(".rigor")
                })
        })
    }
    fn replace_with_env(src: &mut String, env: &HashMap<String, String>) {
        loop {
            let Some(start) = src.find("${") else {
                return;
            };
            let Some(end) = src[start..].find("}") else {
                return;
            };
            let key = &src[start..end + 1];
            let value = env.get(&src[start + 2..end]).cloned().unwrap_or_default();
            *src = src.replace(key, &value);
        }
    }
    pub(crate) fn ensure_env(&mut self, env: &HashMap<String, String>) {
        if self.use_env {
            Self::replace_with_env(&mut self.endpoint, env);
            for test in &mut self.tests {
                Self::replace_with_env(&mut test.route, env);
                // TODO: support env in payload as well
                //Self::replace_with_env(&mut test.payload, env);
                if let Some(ref mut headers) = test.headers {
                    for (_, v) in headers {
                        Self::replace_with_env(v, env);
                    }
                }
            }
        }
    }
    pub(crate) fn init_rigor() -> Self {
        Self {
            tests: vec![
                TestCase {
                    name: "greet-w/o-payload".to_string(),
                    route: "/api/greet".to_string(),
                    method: "GET".to_string(),
                    payload: None,
                    headers: None,
                    expected_status_code: None,
                    skip_payload_fields: Some(vec!["ip".to_string()]),
                },
                TestCase {
                    name: "greet-with-payload".to_string(),
                    route: "/api/greet".to_string(),
                    method: "POST".to_string(),
                    payload: Some(json!({"message":"hello world!"})),
                    headers: Some(BTreeMap::from_iter([(
                        "Content-Type".to_string(),
                        "application/json".to_string(),
                    )])),
                    expected_status_code: Some(200),
                    skip_payload_fields: Some(vec!["ip".to_string()]),
                },
            ],
            endpoint: "${RIGOR_ENDPOINT}".to_string(),
            use_env: true,
        }
    }
}

pub(crate) fn skip_fields(body: &mut serde_json::Value, fields: &Option<Vec<String>>) {
    let Some(fields) = fields.as_ref() else {
        return;
    };
    for field in fields {
        let nesting: Vec<&str> = field.split('.').collect();
        if nesting.len() == 1 {
            if let Some(map) = body.as_object_mut() {
                if let Some(v) = map.get_mut(field) {
                    match v {
                        // set the bool to false
                        Value::Bool(b) => *b = false,
                        // zero the number
                        Value::Number(n) => {
                            let s = if n.is_f64() { "0.0" } else { "0" };
                            *n = serde_json::Number::from_str(s)
                                .expect("unreachable issue with line above or serde_json crate");
                        }
                        // empty the string
                        Value::String(s) => s.clear(),
                        // empty the array map
                        Value::Array(a) => a.clear(),
                        // empty the object map
                        Value::Object(w) => w.clear(),
                        Value::Null => {}
                    }
                }
            }
            continue;
        }
        let m = body.as_object_mut().unwrap();
        let mut v: Option<&mut Value> = m.get_mut(nesting[0]);
        let mut i = 1;
        while i < nesting.len() - 1 {
            let Some(p) = v.take() else {
                // I guess we are fucked
                panic!("bad case walking serde_json::Value");
            };
            v = p.as_object_mut().unwrap().get_mut(nesting[i]);
            i += 1;
        }
        let Some(v) = v.take() else {
            // I guess we are fucked
            panic!("second last element not valid value");
        };
        v.as_object_mut().unwrap().remove(nesting[i]);
    }
}
