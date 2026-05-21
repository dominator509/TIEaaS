use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Deployment profile controls default enforcement behavior and operational
/// expectations. The effective policy can still be overridden by config.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DeploymentProfile {
    Advisory,
    #[default]
    CriticalFailClosed,
    FullFailClosed,
}

/// Stable service identity used across logs, metrics, and signed verdicts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceIdentity {
    pub service_name: String,
    pub environment: String,
    pub region: String,
    pub instance_id: String,
}

impl Default for ServiceIdentity {
    fn default() -> Self {
        Self {
            service_name: "tie".to_string(),
            environment: "dev".to_string(),
            region: "local".to_string(),
            instance_id: "local-instance".to_string(),
        }
    }
}

/// Runtime settings that should remain stable across HTTP, CLI, webhook, and
/// hook-based entrypoints. This is intentionally small so it can be safely
/// shared before the codebase is split into dedicated crates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSettings {
    pub http_bind: String,
    pub grpc_bind: String,
    pub sqlite_url: String,
    pub profile: DeploymentProfile,
    pub verifier_budget: Duration,
    pub registry_cache_ttl: Duration,
    pub validation_cache_ttl: Duration,
    pub require_fact_citations: bool,
    pub require_action_approval: bool,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            http_bind: "127.0.0.1:8080".to_string(),
            grpc_bind: "127.0.0.1:50051".to_string(),
            sqlite_url: "sqlite://tie.db".to_string(),
            profile: DeploymentProfile::CriticalFailClosed,
            verifier_budget: Duration::from_millis(175),
            registry_cache_ttl: Duration::from_secs(300),
            validation_cache_ttl: Duration::from_secs(60),
            require_fact_citations: true,
            require_action_approval: true,
        }
    }
}
