mod rigor;
mod snap;

use core::panic;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

use clap::Parser;
use reqwest::{
    header::{HeaderName, HeaderValue},
    Url,
};
use rigor::Rigor;

/// rigor is a simple application for quick and dirty
/// snapshot testing for rest api, it uses the `.rigor` files
/// to run simple rest api tests
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct App {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    #[clap(alias("i"))]
    Init(Init),
    #[clap(alias("a"))]
    Add(Add),
    #[clap(alias("r"))]
    Run(Run),
}

/// initialize a default rigor file with a few examples
#[derive(Debug, Parser)]
struct Init {
    #[arg(long, action)]
    /// overwrite the rigor file if already present
    force: bool,
    /// Path to the rigor file to use for running the tests
    #[arg(short, long)]
    #[clap(default_value = "test.rigor")]
    path: PathBuf,
}

impl Init {
    pub(crate) fn run(self) {
        let path = Rigor::get_path(self.path.clone().into());
        if path.exists() && !self.force {
            panic!(
                "{} already exists, consider running with `--force` to overwrite",
                path.display()
            );
        }
        std::fs::write(
            path,
            serde_json::to_string_pretty(&rigor::Rigor::init_rigor()).expect("failed to serialize"),
        )
        .expect("failed to write to the path");
    }
}

#[derive(Debug, Parser)]
struct Run {
    /// This provides the endpoint url for RIGOR_ENDPOINT env variable
    /// you can use this instead of manually setting the env variable
    #[arg(short, long, env = "RIGOR_ENDPOINT")]
    url: String,
    /// This forces the endpoint to be against echo server this forces you to update
    /// the snapshots as well if you update the rigor file
    #[arg(short, long)]
    snapshot_dir: Option<PathBuf>,
    /// Path to the rigor file to use for running the tests
    #[arg(short, long)]
    #[clap(default_value = "test.rigor")]
    path: PathBuf,
    /// Overwrite the value for the snapshot
    #[arg(short, long)]
    overwrite: bool,
}
impl Run {
    pub(crate) fn run(self) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut map = HashMap::from_iter(std::env::vars());
        let key = "RIGOR_ENDPOINT".to_string();
        map.insert(key, self.url);

        let path = Rigor::get_path(self.path.clone().into());
        let p = String::from(
            self.path
                .file_name()
                .unwrap()
                .to_str()
                .expect("non unicode file name"),
        ) + ".snapshot";
        let snapshot_path = self.snapshot_dir.or_else(|| self.path.parent().map(PathBuf::from)).map(|s| s.join(&p)).expect("failed to get snapshot path");
        if self.overwrite {
            _ = std::fs::remove_file(&snapshot_path);
        }

        if !path.exists() {
            panic!("path doesn't exists, invalid path: {}", path.display());
        }
        if !path.is_file() {
            panic!("path not file, invalid path: {}", path.display());
        }

        let mut r: rigor::Rigor =
            serde_json::from_slice(&std::fs::read(&path).expect("failed to read a rigor file"))
                .expect("failed to deserialize rigor file");
        r.ensure_env(&map);

        let expected = std::fs::read_to_string(&snapshot_path).ok();
        let mut snapshot = snap::Snapshot { outputs: vec![] };
        // run the rigor file
        for (_i, test) in r.tests.iter().enumerate() {
            let method_str = test.method.clone();
            let endpoint = r.endpoint.clone() + &test.route;
            println!("Testing [{method_str}] URL: {endpoint}");
            let method = match method_str.as_str() {
                "POST" => reqwest::Method::POST,
                "GET" => reqwest::Method::GET,
                "DELETE" => reqwest::Method::DELETE,
                "PUT" => reqwest::Method::PUT,
                "PATCH" => reqwest::Method::PATCH,
                "HEAD" => reqwest::Method::HEAD,
                "TRACE" => reqwest::Method::TRACE,
                _ => {
                    panic!("invalid method {}", test.method)
                }
            };
            let mut req = reqwest::Request::new(
                method,
                Url::from_str(&endpoint).expect("failed to build url from endpoint"),
            );
            let payload = if let Some(v) = &test.payload {
                _ = req.body_mut().insert(reqwest::Body::from(v.to_string()));
                Some(v)
            } else {
                None
            };
            if let Some(h) = &test.headers {
                for (k, v) in h {
                    _ = req.headers_mut().insert(
                        HeaderName::from_bytes(k.as_bytes()).unwrap(),
                        HeaderValue::from_bytes(v.as_bytes()).unwrap(),
                    );
                }
            }
            let client = reqwest::Client::new();
            // run each request serially preferably avoid running stuff in parallel for now!
            let v = match runtime.block_on(async { client.execute(req).await }) {
                Err(err) => panic!("failed to make api request {err:#?}"),
                Ok(v) => v,
            };
            let status_code = v.status().as_u16();
            if !test
                .expected_status_code
                .map(|s| s == status_code)
                .unwrap_or(true)
            {
                panic!("failed to match status_code in rigor file to response status_code");
            }
            let mut body: Option<serde_json::Value> = runtime.block_on(v.json()).ok();
            if let Some(b) = &mut body {
                rigor::skip_fields(b, &test.skip_payload_fields);
            }
            snapshot.outputs.push(snap::Output {
                endpoint,
                method_str,
                request_payload: payload.cloned(),
                status_code,
                response_body: body,
            });
        }
        let src =
            serde_json::to_string_pretty(&snapshot).expect("failed to serialize reqwest::Response");
        if let Some(expected) = expected {
            println!("Comparing results against the saved snapshot");
            pretty_assertions::assert_str_eq!(src, expected);
            println!("Matched successfully");
        } else {
            println!("Storing the new snapshot at: {}", snapshot_path.display());
            std::fs::write(snapshot_path, src).expect("failed to write snapshot to path");
        }
    }
}

pub fn main() {
    let app = App::parse();
    match app.command {
        Commands::Init(i) => i.run(),
        Commands::Add(a) => a.run(),
        Commands::Run(r) => r.run(),
    }
}

#[derive(Parser, Debug)]
/// Add a new test case to the rigor test suite
struct Add {
    /// Path to the rigor file to use for running the tests
    #[arg(short, long)]
    #[clap(default_value = "test.rigor")]
    path: PathBuf,
    #[arg(short, long)]
    /// The name of the test case
    name: String,
    #[arg(short, long)]
    /// The route, is the path relative to the endpoint
    route: String,
    #[arg(short, long)]
    /// The method, defaults to GET
    method: String,
    #[arg(short, long)]
    /// The payload, defaults to empty
    payload: Option<serde_json::Value>,
    #[arg(short, long)]
    /// The headers, defaults to empty
    headers: Option<Vec<String>>,
    #[arg(short, long)]
    /// The expected status code, defaults to 200
    expected_status_code: Option<u16>,
    #[arg(short, long)]
    /// The fields to skip in the payload, defaults to empty
    skip_payload_fields: Option<Vec<String>>,
}

impl Add {
    fn run(&self) {
        let mut rigor = rigor::Rigor::init_rigor();
        rigor.ensure_env(&std::env::vars().collect());
        let test_case = rigor::TestCase {
            route: self.route.clone(),
            method: self.method.clone(),
            payload: self.payload.clone(),
            headers: self.headers.as_ref().map(|h| {
                h.iter()
                    .flat_map(|r| r.split_once(':'))
                    .map(|s| (s.0.trim().to_string(), s.1.trim().to_string()))
                    .collect()
            }),
            expected_status_code: self.expected_status_code.clone(),
            skip_payload_fields: self.skip_payload_fields.clone(),
        };
        rigor.tests.push(test_case);
        _ = std::fs::write(
            Rigor::get_path(self.path.clone().into()),
            serde_json::to_string_pretty(&rigor).expect("failed to serialize"),
        );
    }
}
