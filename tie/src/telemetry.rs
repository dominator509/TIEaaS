use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

/// Telemetry settings shared across local development, docker-compose, and CI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub service_name: String,
    pub environment: String,
    pub json_logs: bool,
    pub env_filter: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: default_service_name().to_string(),
            environment: std::env::var("TIE_ENVIRONMENT").unwrap_or_else(|_| "dev".to_string()),
            json_logs: std::env::var("TIE_JSON_LOGS")
                .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false),
            env_filter: std::env::var("RUST_LOG").unwrap_or_else(|_| {
                "info,sqlx=warn,actix_server=warn,actix_web=warn,hyper=warn".to_string()
            }),
        }
    }
}

pub fn default_service_name() -> &'static str {
    "tie"
}

/// Installs a standard tracing subscriber. This intentionally mirrors the
/// current `main.rs` behavior while giving the repo a stable place to add
/// OpenTelemetry exporters later without bloating the binary entrypoint.
pub fn install_telemetry(config: &TelemetryConfig) {
    let filter = EnvFilter::try_new(config.env_filter.clone())
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,actix_server=warn,actix_web=warn"));

    if config.json_logs {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .json()
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(true)
            .try_init();
    }
}
