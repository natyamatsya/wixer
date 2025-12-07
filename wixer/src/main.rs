pub mod analysis;
pub mod commands;
pub mod common;
pub mod config;
pub mod constants;
pub mod describe_db;
pub mod project;
pub mod schema_mapping;
pub mod dirs;
mod utils;
mod validate;
mod wix_to_yaml;
mod xsd_to_json;
mod yaml_to_wix;
mod wix_exit_codes;
pub mod wixpdb;
mod app;

#[tokio::main]
async fn main() {
    if let Err(err) = app::run().await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}
