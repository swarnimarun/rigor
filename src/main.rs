mod rigor;
mod snap;

use core::panic;
use std::{collections::HashMap, str::FromStr};

use clap::Parser;
use reqwest::{
    header::{HeaderName, HeaderValue},
    Url,
};

/// rigor is a simple application for quick and dirty
/// snapshot testing for rest api, it uses the `*.rigor` files in your current directory
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
    //#[clap(alias("a"))]
    //Add(Add),
    #[clap(alias("r"))]
    Run(Run),
}

/// initialize a default rigor file with a few examples
#[derive(Debug, Parser)]
struct Init {
    #[arg(long, action)]
    /// overwrite the rigor file if already present
    force: bool,
}

impl Init {
    pub(crate) fn run(self) {
        let path = std::env::current_dir()
            .expect("failed to get current directory")
            .join(".rigor");
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

/// add a test to the rigor file using an interactive cli interface
//#[derive(Debug, Parser)]
//struct Add;
//impl Add {
//    pub(crate) fn run(self) {
//        //
//    }
//}

#[derive(Debug, Parser)]
struct Run {
    #[clap(default_value = "https://echo.free.beeceptor.com")]
    url: String,
}
impl Run {
    pub(crate) fn run(self) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut map = HashMap::from_iter(std::env::vars());
        let key = "RIGOR_ENDPOINT".to_string();
        if !map.contains_key(&key) {
            map.insert(key, self.url);
        }

        let cwd = std::env::current_dir().expect("failed to get the current directory");
        for e in cwd.read_dir().expect("failed to read the dir") {
            let de = e.expect("failed to read the directory entry");
            let Ok(ft) = de.file_type() else { continue };
            if ft.is_file() && de.file_name().as_encoded_bytes().ends_with(b".rigor") {
                let mut r: rigor::Rigor = serde_json::from_slice(
                    &std::fs::read(de.path()).expect("failed to read a `*.rigor` file"),
                )
                .expect("failed to deserialize .rigor file");
                r.ensure_env(&map);
                let expected = std::fs::read_to_string(de.path().with_extension("snapshot")).ok();
                //let snapshot: Option<snap::Snapshot> =
                //    expected.and_then(|b| serde_json::from_str(&b).ok());
                let mut snapshot = snap::Snapshot { outputs: vec![] };
                // run the rigor file
                for (_i, test) in r.tests.iter().enumerate() {
                    let method = match test.method.as_str() {
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
                        Url::from_str(&r.endpoint).expect("failed to build url from endpoint"),
                    );
                    if let Some(h) = &test.headers {
                        for (k, v) in h {
                            _ = req.headers_mut().insert(
                                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                                HeaderValue::from_bytes(v.as_bytes()).unwrap(),
                            );
                        }
                    }
                    let client = reqwest::Client::new();
                    // run each request serially preferrably avoid running stuff in parrallel for
                    // now!
                    let Ok(v) = runtime.block_on(async { client.execute(req).await }) else {
                        panic!("failed to make api request");
                    };
                    let status_code = v.status().as_u16();
                    let mut body: serde_json::Value = runtime
                        .block_on(v.json())
                        .expect("failed to read the body from the response");
                    rigor::skip_fields(&mut body, &test.skip_payload_fields);
                    snapshot.outputs.push(snap::Output { status_code, body });
                }
                let src = serde_json::to_string_pretty(&snapshot)
                    .expect("failed to serialize reqwest::Response");
                if let Some(expected) = expected {
                    pretty_assertions::assert_str_eq!(src, expected);
                } else {
                    std::fs::write(de.path().with_extension("snapshot"), src)
                        .expect("failed to write");
                }
            }
        }
    }
}

pub fn main() {
    let app = App::parse();
    match app.command {
        Commands::Init(i) => i.run(),
        //Commands::Add(a) => a.run(),
        Commands::Run(r) => r.run(),
    }
}
