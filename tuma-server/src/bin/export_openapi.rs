//! Exports the OpenAPI contract to a file without booting the server.
//!
//! Usage: cargo run --bin export_openapi -- <output.json>
//!
//! Wired into the dev runner as `./run.sh openapi`, which writes the spec
//! to tuma-platform/openapi.json for the platform's client codegen (Orval).

use tuma_server::api_doc::ApiDoc;
use utoipa::OpenApi;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../tuma-platform/openapi.json".to_string());

    let json = ApiDoc::openapi().to_pretty_json()?;
    std::fs::write(&out, json)?;

    println!("OpenAPI spec written to {out}");
    Ok(())
}
