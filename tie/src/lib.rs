//! TIE library seam for the Phase 0/1 modular monolith.
//!
//! The current executable remains centered in `src/main.rs`, but these modules
//! establish the extraction path toward a reusable library crate and, later,
//! a workspace split if warranted by scale.

pub mod app_state;
pub mod bootstrap;
pub mod telemetry;

pub use app_state::{DeploymentProfile, RuntimeSettings, ServiceIdentity};
pub use bootstrap::{BootReport, DatabaseBootstrap};
pub use telemetry::{default_service_name, install_telemetry, TelemetryConfig};
