use actix_web::{
    delete, get, post, put,
    web::{Data, Json, Path, Query},
    App, HttpResponse, HttpServer, ResponseError,
};
use ahash::AHasher;
use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signer, SigningKey};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, SqlitePool};
use std::hash::{Hash, Hasher};
use std::{collections::BTreeMap, fmt, sync::Arc, time::Duration};
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};
use tracing_subscriber::EnvFilter;
#[cfg(feature = "swagger-ui")]
use utoipa::{OpenApi, ToSchema};
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;
#[derive(Debug, Parser)]
#[command(name = "tie", version, about = "TIE validation and registry service")]
struct Cli {
    #[arg(long, env = "TIE_DATABASE_URL", default_value = "sqlite://tie.db")]
    database_url: String,
    #[arg(long, env = "TIE_HTTP_BIND", default_value = "127.0.0.1:8080")]
    http_bind: String,
    #[arg(long, env = "TIE_GRPC_BIND", default_value = "127.0.0.1:50051")]
    grpc_bind: String,
    #[arg(long, env = "TIE_ENABLE_GRPC", default_value_t = false)]
    enable_grpc: bool,
    #[arg(long, env = "TIE_POLICY_MODE", default_value = "critical-fail-closed")]
    policy_mode: PolicyMode,
    #[arg(long, env = "TIE_SIGNING_KEY_HEX")]
    signing_key_hex: Option<String>,
    #[arg(long, env = "TIE_CACHE_TTL_SECS", default_value_t = 300)]
    cache_ttl_secs: u64,
    #[arg(long, env = "TIE_VALIDATION_CACHE_TTL_SECS", default_value_t = 60)]
    validation_cache_ttl_secs: u64,
    #[arg(long, env = "TIE_VERIFIER_BUDGET_MS", default_value_t = 175)]
    verifier_budget_ms: u64,
    #[arg(long, env = "TIE_REQUIRE_FACT_CITATIONS", default_value_t = true)]
    require_fact_citations: bool,
    #[arg(long, env = "TIE_REQUIRE_ACTION_APPROVAL", default_value_t = true)]
    require_action_approval: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
enum PolicyMode {
    Advisory,
    CriticalFailClosed,
    FullFailClosed,
}
impl std::str::FromStr for PolicyMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "advisory" => Ok(Self::Advisory),
            "critical-fail-closed" | "critical_fail_closed" => Ok(Self::CriticalFailClosed),
            "full-fail-closed" | "full_fail_closed" => Ok(Self::FullFailClosed),
            other => Err(format!(
                "unsupported policy mode '{other}'. expected advisory, critical-fail-closed, or full-fail-closed"
            )),
        }
    }
}
impl fmt::Display for PolicyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Advisory => write!(f, "advisory"),
            Self::CriticalFailClosed => write!(f, "critical-fail-closed"),
            Self::FullFailClosed => write!(f, "full-fail-closed"),
        }
    }
}
#[derive(Debug, Subcommand)]
enum Commands {
    Serve,
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
}
#[derive(Debug, Subcommand)]
enum RegistryCommand {
    Create {
        #[arg(long)]
        namespace: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
        #[arg(long, default_value = "{}")]
        provenance: String,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    Get {
        #[arg(long)]
        id: String,
    },
    List {
        #[arg(long, default_value_t = false)]
        include_retired: bool,
    },
    Delete {
        #[arg(long)]
        id: String,
    },
}
#[derive(Debug, Clone)]
struct AppConfig {
    http_bind: String,
    grpc_bind: String,
    enable_grpc: bool,
    policy_mode: PolicyMode,
    verifier_budget: Duration,
    require_fact_citations: bool,
    require_action_approval: bool,
}
#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    registry_cache: Cache<String, RegistryRecord>,
    validation_cache: Cache<String, ValidationResponse>,
    config: AppConfig,
    signing_key: Option<Arc<SigningKey>>,
}
#[derive(Debug, Error)]
enum AppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("timeout: {0}")]
    #[allow(dead_code)]
    Timeout(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}
impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status, code, retryable) = match self {
            Self::InvalidInput(_) => (
                actix_web::http::StatusCode::BAD_REQUEST,
                "invalid_input",
                false,
            ),
            Self::NotFound(_) => (actix_web::http::StatusCode::NOT_FOUND, "not_found", false),
            Self::Timeout(_) => (
                actix_web::http::StatusCode::GATEWAY_TIMEOUT,
                "timeout",
                true,
            ),
            Self::Database(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "database_error",
                true,
            ),
            Self::Internal(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                true,
            ),
        };
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: code.to_string(),
                message: self.to_string(),
                retryable,
                request_id: Uuid::now_v7().to_string(),
            },
        };
        HttpResponse::build(status).json(body)
    }
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct ErrorEnvelope {
    error: ErrorBody,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct ErrorBody {
    code: String,
    message: String,
    retryable: bool,
    request_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct RegistryRecord {
    id: String,
    namespace: String,
    kind: String,
    key: String,
    version: i64,
    value: Value,
    provenance: Value,
    digest_sha256: String,
    signature_ed25519: Option<String>,
    created_at: String,
    updated_at: String,
    retired_at: Option<String>,
    tags: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct RegistryRecordUpsert {
    namespace: String,
    kind: String,
    key: String,
    value: Value,
    #[serde(default = "default_empty_object")]
    provenance: Value,
    #[serde(default)]
    tags: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct RegistryRecordUpdate {
    value: Value,
    #[serde(default = "default_empty_object")]
    provenance: Value,
    #[serde(default)]
    tags: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct ListRegistryQuery {
    #[serde(default)]
    include_retired: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
enum SubjectType {
    Code,
    Fact,
    Action,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
enum Verdict {
    Pass,
    Warn,
    Fail,
    Inconclusive,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct ValidationRequest {
    #[serde(default = "new_request_id")]
    request_id: String,
    subject_type: SubjectType,
    subject: String,
    #[serde(default)]
    registry_record_ids: Vec<String>,
    #[serde(default = "default_empty_object")]
    metadata: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct EvidenceItem {
    adapter: String,
    verdict: Verdict,
    severity: Severity,
    message: String,
    score: f32,
    references: Vec<String>,
    duration_ms: u128,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "swagger-ui", derive(ToSchema))]
struct ValidationResponse {
    request_id: String,
    verdict: Verdict,
    enforcement_action: String,
    summary: String,
    evidence: Vec<EvidenceItem>,
    registry_context: Vec<RegistryRecord>,
    cache_hit: bool,
    timings_ms: BTreeMap<String, u128>,
}
#[derive(Debug, Clone)]
struct KaizenEvent {
    request_id: String,
    category: String,
    severity: Severity,
    component: String,
    message: String,
    metadata: Value,
}
#[derive(Debug, FromRow)]
struct RegistryRow {
    id: String,
    namespace: String,
    kind: String,
    key: String,
    version: i64,
    value_json: String,
    provenance_json: String,
    digest_sha256: String,
    signature_ed25519: Option<String>,
    created_at: String,
    updated_at: String,
    retired_at: Option<String>,
    tags_json: String,
}
#[cfg(feature = "swagger-ui")]
#[derive(OpenApi)]
#[openapi(
    paths(
        healthz,
        readyz,
        validate,
        create_registry_record,
        list_registry_records,
        get_registry_record,
        get_registry_record_by_key,
        update_registry_record,
        delete_registry_record
    ),
    components(
        schemas(
            PolicyMode,
            RegistryRecord,
            RegistryRecordUpsert,
            RegistryRecordUpdate,
            ValidationRequest,
            ValidationResponse,
            EvidenceItem,
            ErrorEnvelope,
            ErrorBody,
            SubjectType,
            Severity,
            Verdict
        )
    ),
    tags(
        (name = "tie", description = "Trust, Integrity, and Evidence service API")
    )
)]
struct ApiDoc;
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if let Err(error) = run().await {
        error!(error = %error, "TIE service terminated with an error");
        return Err(std::io::Error::other(error.to_string()));
    }
    Ok(())
}
async fn run() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let signing_key = match cli.signing_key_hex.as_deref() {
        Some(raw) => Some(Arc::new(parse_signing_key(raw)?)),
        None => None,
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .connect(&cli.database_url)
        .await
        .with_context(|| format!("failed to connect to database {}", cli.database_url))?;
    bootstrap_schema(&pool).await?;
    let state = AppState {
        pool,
        registry_cache: Cache::builder()
            .time_to_live(Duration::from_secs(cli.cache_ttl_secs))
            .max_capacity(10_000)
            .build(),
        validation_cache: Cache::builder()
            .time_to_live(Duration::from_secs(cli.validation_cache_ttl_secs))
            .max_capacity(25_000)
            .build(),
        config: AppConfig {
            http_bind: cli.http_bind.clone(),
            grpc_bind: cli.grpc_bind.clone(),
            enable_grpc: cli.enable_grpc,
            policy_mode: cli.policy_mode,
            verifier_budget: Duration::from_millis(cli.verifier_budget_ms),
            require_fact_citations: cli.require_fact_citations,
            require_action_approval: cli.require_action_approval,
        },
        signing_key,
    };
    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => serve(state).await,
        Commands::Registry { command } => run_registry_cli(state, command).await,
    }
}
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,sqlx=warn,actix_server=warn,actix_web=warn,hyper=warn")
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .try_init();
}
async fn serve(state: AppState) -> anyhow::Result<()> {
    if state.config.enable_grpc {
        spawn_grpc_boundary(state.config.grpc_bind.clone()).await;
    }
    info!(
        http_bind = %state.config.http_bind,
        grpc_enabled = state.config.enable_grpc,
        grpc_bind = %state.config.grpc_bind,
        policy_mode = %state.config.policy_mode,
        "starting TIE HTTP server"
    );
    let shared_state = Data::new(state.clone());
    HttpServer::new(move || {
        let app = App::new()
            .app_data(shared_state.clone())
            .service(healthz)
            .service(readyz)
            .service(validate)
            .service(create_registry_record)
            .service(list_registry_records)
            .service(get_registry_record)
            .service(get_registry_record_by_key)
            .service(update_registry_record)
            .service(delete_registry_record);
        #[cfg(feature = "swagger-ui")]
        let app = app.service(
            SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()),
        );
        app
    })
    .bind(&state.config.http_bind)?
    .run()
    .await?;
    Ok(())
}
#[cfg(feature = "grpc")]
async fn spawn_grpc_boundary(bind: String) {
    tokio::spawn(async move {
        warn!(
            grpc_bind = %bind,
            "gRPC feature is enabled, but the protobuf service surface is intentionally deferred to the next phase. HTTP is authoritative in this build."
        );
    });
}
#[cfg(not(feature = "grpc"))]
async fn spawn_grpc_boundary(bind: String) {
    warn!(
        grpc_bind = %bind,
        "TIE was asked to enable gRPC, but the binary was built without the grpc feature"
    );
}
async fn run_registry_cli(state: AppState, command: RegistryCommand) -> anyhow::Result<()> {
    match command {
        RegistryCommand::Create {
            namespace,
            kind,
            key,
            value,
            provenance,
            tags,
        } => {
            let input = RegistryRecordUpsert {
                namespace,
                kind,
                key,
                value: serde_json::from_str(&value).context("registry value must be valid JSON")?,
                provenance: serde_json::from_str(&provenance)
                    .context("registry provenance must be valid JSON")?,
                tags,
            };
            let record = create_record(&state, input).await?;
            println!("{}", serde_json::to_string_pretty(&record)?);
        }
        RegistryCommand::Get { id } => {
            let record = get_record_by_id(&state, &id).await?;
            println!("{}", serde_json::to_string_pretty(&record)?);
        }
        RegistryCommand::List { include_retired } => {
            let records = list_records(&state, include_retired).await?;
            println!("{}", serde_json::to_string_pretty(&records)?);
        }
        RegistryCommand::Delete { id } => {
            soft_delete_record(&state, &id).await?;
            println!("deleted {id}");
        }
    }
    Ok(())
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/healthz",
    tag = "tie",
    responses((status = 200, description = "Liveness probe"))
))]
#[get("/healthz")]
async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok", "service": "tie"}))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/readyz",
    tag = "tie",
    responses((status = 200, description = "Readiness probe"))
))]
#[get("/readyz")]
async fn readyz(state: Data<AppState>) -> Result<Json<Value>, AppError> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?;
    Ok(Json(json!({
        "status": "ready",
        "policy_mode": state.config.policy_mode.to_string(),
        "grpc_configured": state.config.enable_grpc,
    })))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/v1/validate",
    tag = "tie",
    request_body = ValidationRequest,
    responses(
        (status = 200, description = "Validation response", body = ValidationResponse),
        (status = 400, description = "Invalid input", body = ErrorEnvelope)
    )
))]
#[post("/v1/validate")]
#[instrument(skip_all, fields(request_id = %payload.request_id))]
async fn validate(
    state: Data<AppState>,
    payload: Json<ValidationRequest>,
) -> Result<Json<ValidationResponse>, AppError> {
    let mut request = payload.into_inner();
    if request.subject.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "subject must not be empty".to_string(),
        ));
    }
    if request.request_id.trim().is_empty() {
        request.request_id = new_request_id();
    }
    let cache_key = validation_cache_key(&request)?;
    if let Some(mut cached) = state.validation_cache.get(&cache_key).await {
        cached.cache_hit = true;
        return Ok(Json(cached));
    }
    let started = std::time::Instant::now();
    let registry_context = load_registry_context(&state, &request.registry_record_ids).await?;
    let mut evidence = Vec::new();
    let registry_refs: Vec<String> = registry_context.iter().map(|r| r.id.clone()).collect();
    let code_ev = run_with_budget(
        state.config.verifier_budget,
        run_code_verifier(&request, &registry_refs),
        "code_verifier",
    )
    .await;
    if let Some(item) = code_ev {
        evidence.push(item);
    }
    let fact_ev = run_with_budget(
        state.config.verifier_budget,
        run_fact_verifier(
            &request,
            &registry_refs,
            state.config.require_fact_citations,
        ),
        "fact_verifier",
    )
    .await;
    if let Some(item) = fact_ev {
        evidence.push(item);
    }
    let action_ev = run_with_budget(
        state.config.verifier_budget,
        run_action_verifier(
            &request,
            &registry_refs,
            state.config.require_action_approval,
        ),
        "action_verifier",
    )
    .await;
    if let Some(item) = action_ev {
        evidence.push(item);
    }
    let verdict = resolve_verdict(state.config.policy_mode, &evidence);
    let enforcement_action = match verdict {
        Verdict::Pass => "allow",
        Verdict::Warn => "allow_with_warning",
        Verdict::Fail => "block",
        Verdict::Inconclusive => "quarantine",
    }
    .to_string();
    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("total".to_string(), started.elapsed().as_millis());
    let summary = format!(
        "validation {} with {} evidence item(s) under policy mode {}",
        verdict_label(&verdict),
        evidence.len(),
        state.config.policy_mode
    );
    let response = ValidationResponse {
        request_id: request.request_id.clone(),
        verdict: verdict.clone(),
        enforcement_action,
        summary,
        evidence: evidence.clone(),
        registry_context,
        cache_hit: false,
        timings_ms,
    };
    state
        .validation_cache
        .insert(cache_key, response.clone())
        .await;
    if verdict != Verdict::Pass {
        log_kaizen_event(
            &state,
            KaizenEvent {
                request_id: request.request_id,
                category: "validation_outcome".to_string(),
                severity: highest_severity(&evidence),
                component: "decision_layer".to_string(),
                message: format!("validation finished with {}", verdict_label(&verdict)),
                metadata: json!({
                    "verdict": verdict_label(&verdict),
                    "subject_type": request.subject_type,
                    "evidence_count": evidence.len(),
                }),
            },
        )
        .await?;
    }
    Ok(Json(response))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    post,
    path = "/v1/registry/records",
    tag = "tie",
    request_body = RegistryRecordUpsert,
    responses(
        (status = 200, description = "Created registry record", body = RegistryRecord),
        (status = 400, description = "Invalid input", body = ErrorEnvelope)
    )
))]
#[post("/v1/registry/records")]
async fn create_registry_record(
    state: Data<AppState>,
    payload: Json<RegistryRecordUpsert>,
) -> Result<Json<RegistryRecord>, AppError> {
    let record = create_record(&state, payload.into_inner()).await?;
    Ok(Json(record))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/v1/registry/records",
    tag = "tie",
    params(("include_retired" = bool, Query, description = "Include retired records")),
    responses((status = 200, description = "List registry records", body = [RegistryRecord]))
))]
#[get("/v1/registry/records")]
async fn list_registry_records(
    state: Data<AppState>,
    query: Query<ListRegistryQuery>,
) -> Result<Json<Vec<RegistryRecord>>, AppError> {
    let records = list_records(&state, query.include_retired).await?;
    Ok(Json(records))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/v1/registry/records/{id}",
    tag = "tie",
    params(("id" = String, Path, description = "Registry record id")),
    responses(
        (status = 200, description = "Registry record", body = RegistryRecord),
        (status = 404, description = "Not found", body = ErrorEnvelope)
    )
))]
#[get("/v1/registry/records/{id}")]
async fn get_registry_record(
    state: Data<AppState>,
    id: Path<String>,
) -> Result<Json<RegistryRecord>, AppError> {
    let record = get_record_by_id(&state, &id).await?;
    Ok(Json(record))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    get,
    path = "/v1/registry/records/by-key/{namespace}/{kind}/{key}",
    tag = "tie",
    params(
        ("namespace" = String, Path, description = "Registry namespace"),
        ("kind" = String, Path, description = "Registry kind"),
        ("key" = String, Path, description = "Registry key")
    ),
    responses((status = 200, description = "Latest active record by key", body = RegistryRecord))
))]
#[get("/v1/registry/records/by-key/{namespace}/{kind}/{key}")]
async fn get_registry_record_by_key(
    state: Data<AppState>,
    params: Path<(String, String, String)>,
) -> Result<Json<RegistryRecord>, AppError> {
    let (namespace, kind, key) = params.into_inner();
    let record = get_latest_record_by_key(&state, &namespace, &kind, &key).await?;
    Ok(Json(record))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    put,
    path = "/v1/registry/records/{id}",
    tag = "tie",
    request_body = RegistryRecordUpdate,
    params(("id" = String, Path, description = "Registry record id to supersede")),
    responses((status = 200, description = "Superseded record", body = RegistryRecord))
))]
#[put("/v1/registry/records/{id}")]
async fn update_registry_record(
    state: Data<AppState>,
    id: Path<String>,
    payload: Json<RegistryRecordUpdate>,
) -> Result<Json<RegistryRecord>, AppError> {
    let record = supersede_record(&state, &id, payload.into_inner()).await?;
    Ok(Json(record))
}
#[cfg_attr(feature = "swagger-ui", utoipa::path(
    delete,
    path = "/v1/registry/records/{id}",
    tag = "tie",
    params(("id" = String, Path, description = "Registry record id to retire")),
    responses((status = 200, description = "Retirement result"), (status = 404, description = "Not found", body = ErrorEnvelope))
))]
#[delete("/v1/registry/records/{id}")]
async fn delete_registry_record(
    state: Data<AppState>,
    id: Path<String>,
) -> Result<Json<Value>, AppError> {
    soft_delete_record(&state, &id).await?;
    Ok(Json(json!({"status": "retired", "id": id.into_inner()})))
}
async fn bootstrap_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS registry_records (
            id TEXT PRIMARY KEY,
            namespace TEXT NOT NULL,
            kind TEXT NOT NULL,
            key TEXT NOT NULL,
            version INTEGER NOT NULL,
            value_json TEXT NOT NULL,
            provenance_json TEXT NOT NULL,
            digest_sha256 TEXT NOT NULL,
            signature_ed25519 TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            retired_at TEXT,
            tags_json TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_registry_key_version
        ON registry_records(namespace, kind, key, version DESC);
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS kaizen_events (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            request_id TEXT NOT NULL,
            category TEXT NOT NULL,
            severity TEXT NOT NULL,
            component TEXT NOT NULL,
            message TEXT NOT NULL,
            metadata_json TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_kaizen_events_created_at
        ON kaizen_events(created_at DESC);
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}
async fn create_record(
    state: &AppState,
    input: RegistryRecordUpsert,
) -> Result<RegistryRecord, AppError> {
    validate_registry_input(&input.namespace, &input.kind, &input.key)?;
    let next_version = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(version) FROM registry_records WHERE namespace = ?1 AND kind = ?2 AND key = ?3",
    )
    .bind(&input.namespace)
    .bind(&input.kind)
    .bind(&input.key)
    .fetch_one(&state.pool)
    .await?
    .unwrap_or(0)
        + 1;
    let now = Utc::now().to_rfc3339();
    let id = Uuid::now_v7().to_string();
    let value_json = serde_json::to_string(&input.value)
        .map_err(|error| AppError::InvalidInput(format!("invalid registry value: {error}")))?;
    let provenance_json = serde_json::to_string(&input.provenance)
        .map_err(|error| AppError::InvalidInput(format!("invalid provenance value: {error}")))?;
    let tags_json = serde_json::to_string(&input.tags)
        .map_err(|error| AppError::InvalidInput(format!("invalid tags: {error}")))?;
    let digest = registry_digest(
        &input.namespace,
        &input.kind,
        &input.key,
        next_version,
        &value_json,
    );
    let signature = sign_digest(state.signing_key.as_deref(), &digest);
    sqlx::query(
        r#"
        INSERT INTO registry_records (
            id, namespace, kind, key, version,
            value_json, provenance_json, digest_sha256, signature_ed25519,
            created_at, updated_at, retired_at, tags_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12)
        "#,
    )
    .bind(&id)
    .bind(&input.namespace)
    .bind(&input.kind)
    .bind(&input.key)
    .bind(next_version)
    .bind(&value_json)
    .bind(&provenance_json)
    .bind(&digest)
    .bind(signature.as_deref())
    .bind(&now)
    .bind(&now)
    .bind(&tags_json)
    .execute(&state.pool)
    .await?;
    state.registry_cache.invalidate(&id).await;
    state.validation_cache.invalidate_all();
    get_record_by_id(state, &id).await
}
async fn list_records(
    state: &AppState,
    include_retired: bool,
) -> Result<Vec<RegistryRecord>, AppError> {
    let rows = if include_retired {
        sqlx::query_as::<_, RegistryRow>(
            r#"
            SELECT id, namespace, kind, key, version, value_json, provenance_json,
                   digest_sha256, signature_ed25519, created_at, updated_at, retired_at, tags_json
            FROM registry_records
            ORDER BY namespace, kind, key, version DESC
            "#,
        )
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, RegistryRow>(
            r#"
            SELECT id, namespace, kind, key, version, value_json, provenance_json,
                   digest_sha256, signature_ed25519, created_at, updated_at, retired_at, tags_json
            FROM registry_records
            WHERE retired_at IS NULL
            ORDER BY namespace, kind, key, version DESC
            "#,
        )
        .fetch_all(&state.pool)
        .await?
    };
    rows.into_iter().map(TryInto::try_into).collect()
}
async fn get_record_by_id(state: &AppState, id: &str) -> Result<RegistryRecord, AppError> {
    if let Some(record) = state.registry_cache.get(id).await {
        return Ok(record);
    }
    let row = sqlx::query_as::<_, RegistryRow>(
        r#"
        SELECT id, namespace, kind, key, version, value_json, provenance_json,
               digest_sha256, signature_ed25519, created_at, updated_at, retired_at, tags_json
        FROM registry_records
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("registry record {id}")))?;
    let record: RegistryRecord = row.try_into()?;
    state
        .registry_cache
        .insert(id.to_string(), record.clone())
        .await;
    Ok(record)
}
async fn get_latest_record_by_key(
    state: &AppState,
    namespace: &str,
    kind: &str,
    key: &str,
) -> Result<RegistryRecord, AppError> {
    let row = sqlx::query_as::<_, RegistryRow>(
        r#"
        SELECT id, namespace, kind, key, version, value_json, provenance_json,
               digest_sha256, signature_ed25519, created_at, updated_at, retired_at, tags_json
        FROM registry_records
        WHERE namespace = ?1 AND kind = ?2 AND key = ?3 AND retired_at IS NULL
        ORDER BY version DESC
        LIMIT 1
        "#,
    )
    .bind(namespace)
    .bind(kind)
    .bind(key)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("registry record {namespace}/{kind}/{key}")))?;
    let record: RegistryRecord = row.try_into()?;
    state
        .registry_cache
        .insert(record.id.clone(), record.clone())
        .await;
    Ok(record)
}
async fn supersede_record(
    state: &AppState,
    old_id: &str,
    input: RegistryRecordUpdate,
) -> Result<RegistryRecord, AppError> {
    let current = get_record_by_id(state, old_id).await?;
    let retired_at = Utc::now().to_rfc3339();
    sqlx::query("UPDATE registry_records SET retired_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind(&retired_at)
        .bind(old_id)
        .execute(&state.pool)
        .await?;
    state.registry_cache.invalidate(old_id).await;
    let new_record = create_record(
        state,
        RegistryRecordUpsert {
            namespace: current.namespace,
            kind: current.kind,
            key: current.key,
            value: input.value,
            provenance: input.provenance,
            tags: input.tags,
        },
    )
    .await?;
    Ok(new_record)
}
async fn soft_delete_record(state: &AppState, id: &str) -> Result<(), AppError> {
    let retired_at = Utc::now().to_rfc3339();
    let rows_affected =
        sqlx::query("UPDATE registry_records SET retired_at = ?1, updated_at = ?1 WHERE id = ?2")
            .bind(&retired_at)
            .bind(id)
            .execute(&state.pool)
            .await?
            .rows_affected();
    if rows_affected == 0 {
        return Err(AppError::NotFound(format!("registry record {id}")));
    }
    state.registry_cache.invalidate(id).await;
    state.validation_cache.invalidate_all();
    Ok(())
}
async fn load_registry_context(
    state: &AppState,
    ids: &[String],
) -> Result<Vec<RegistryRecord>, AppError> {
    let mut records = Vec::with_capacity(ids.len());
    for id in ids {
        records.push(get_record_by_id(state, id).await?);
    }
    Ok(records)
}
async fn run_with_budget<F>(
    budget: Duration,
    fut: F,
    adapter_name: &'static str,
) -> Option<EvidenceItem>
where
    F: std::future::Future<Output = Option<EvidenceItem>>,
{
    match tokio::time::timeout(budget, fut).await {
        Ok(item) => item,
        Err(_) => Some(EvidenceItem {
            adapter: adapter_name.to_string(),
            verdict: Verdict::Inconclusive,
            severity: Severity::Error,
            message: format!(
                "adapter exceeded verifier budget of {}ms and was downgraded to inconclusive",
                budget.as_millis()
            ),
            score: 0.0,
            references: Vec::new(),
            duration_ms: budget.as_millis(),
        }),
    }
}
async fn run_code_verifier(
    request: &ValidationRequest,
    registry_refs: &[String],
) -> Option<EvidenceItem> {
    if !matches!(request.subject_type, SubjectType::Code) {
        return None;
    }
    let started = std::time::Instant::now();
    let subject = request.subject.to_ascii_lowercase();
    let (verdict, severity, score, message) = if subject.contains("std::process::command")
        || subject.contains("exec(")
        || subject.contains("rm -rf")
        || subject.contains("unsafe ")
    {
        (
            Verdict::Fail,
            Severity::Critical,
            0.05,
            "dangerous code pattern detected in subject".to_string(),
        )
    } else if subject.contains("todo") || subject.contains("fixme") {
        (
            Verdict::Warn,
            Severity::Warning,
            0.65,
            "code contains unresolved TODO/FIXME markers".to_string(),
        )
    } else {
        (
            Verdict::Pass,
            Severity::Info,
            0.97,
            "code heuristics passed".to_string(),
        )
    };
    Some(EvidenceItem {
        adapter: "code_verifier".to_string(),
        verdict,
        severity,
        message,
        score,
        references: registry_refs.to_vec(),
        duration_ms: started.elapsed().as_millis(),
    })
}
async fn run_fact_verifier(
    request: &ValidationRequest,
    registry_refs: &[String],
    require_citations: bool,
) -> Option<EvidenceItem> {
    if !matches!(request.subject_type, SubjectType::Fact) {
        return None;
    }
    let started = std::time::Instant::now();
    let citations = request
        .metadata
        .get("citations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let emphatic_language = request.subject.contains("always") || request.subject.contains("never");
    let (verdict, severity, score, message) = if require_citations && citations.is_empty() {
        (
            Verdict::Fail,
            Severity::Error,
            0.15,
            "fact claim requires citations but none were provided".to_string(),
        )
    } else if emphatic_language {
        (
            Verdict::Warn,
            Severity::Warning,
            0.70,
            "claim uses absolute language and should be reviewed carefully".to_string(),
        )
    } else {
        (
            Verdict::Pass,
            Severity::Info,
            0.93,
            format!("fact claim validated with {} citation(s)", citations.len()),
        )
    };
    Some(EvidenceItem {
        adapter: "fact_verifier".to_string(),
        verdict,
        severity,
        message,
        score,
        references: registry_refs.to_vec(),
        duration_ms: started.elapsed().as_millis(),
    })
}
async fn run_action_verifier(
    request: &ValidationRequest,
    registry_refs: &[String],
    require_action_approval: bool,
) -> Option<EvidenceItem> {
    if !matches!(request.subject_type, SubjectType::Action) {
        return None;
    }
    let started = std::time::Instant::now();
    let subject = request.subject.to_ascii_lowercase();
    let approval_token = request
        .metadata
        .get("approval_token")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (verdict, severity, score, message) = if subject.contains("delete production")
        || subject.contains("wire transfer")
        || subject.contains("disable auth")
    {
        (
            Verdict::Fail,
            Severity::Critical,
            0.05,
            "high-risk action blocked by policy".to_string(),
        )
    } else if require_action_approval && approval_token.is_empty() {
        (
            Verdict::Warn,
            Severity::Warning,
            0.55,
            "action is missing approval_token metadata".to_string(),
        )
    } else {
        (
            Verdict::Pass,
            Severity::Info,
            0.95,
            "action policy checks passed".to_string(),
        )
    };
    Some(EvidenceItem {
        adapter: "action_verifier".to_string(),
        verdict,
        severity,
        message,
        score,
        references: registry_refs.to_vec(),
        duration_ms: started.elapsed().as_millis(),
    })
}
fn resolve_verdict(policy_mode: PolicyMode, evidence: &[EvidenceItem]) -> Verdict {
    if evidence.is_empty() {
        return Verdict::Inconclusive;
    }
    let has_fail = evidence.iter().any(|item| item.verdict == Verdict::Fail);
    let has_warn = evidence.iter().any(|item| item.verdict == Verdict::Warn);
    let has_critical = evidence
        .iter()
        .any(|item| item.verdict == Verdict::Fail && item.severity == Severity::Critical);
    match policy_mode {
        PolicyMode::Advisory => {
            if has_fail || has_warn {
                Verdict::Warn
            } else {
                Verdict::Pass
            }
        }
        PolicyMode::CriticalFailClosed => {
            if has_critical {
                Verdict::Fail
            } else if has_warn || has_fail {
                Verdict::Warn
            } else {
                Verdict::Pass
            }
        }
        PolicyMode::FullFailClosed => {
            if has_fail {
                Verdict::Fail
            } else if has_warn {
                Verdict::Warn
            } else {
                Verdict::Pass
            }
        }
    }
}
fn highest_severity(evidence: &[EvidenceItem]) -> Severity {
    if evidence.iter().any(|e| e.severity == Severity::Critical) {
        Severity::Critical
    } else if evidence.iter().any(|e| e.severity == Severity::Error) {
        Severity::Error
    } else if evidence.iter().any(|e| e.severity == Severity::Warning) {
        Severity::Warning
    } else {
        Severity::Info
    }
}
async fn log_kaizen_event(state: &AppState, event: KaizenEvent) -> Result<(), AppError> {
    let id = Uuid::now_v7().to_string();
    let created_at = Utc::now().to_rfc3339();
    let metadata_json = serde_json::to_string(&event.metadata).map_err(|error| {
        AppError::Internal(format!("failed to serialize kaizen metadata: {error}"))
    })?;
    debug!(
        request_id = %event.request_id,
        category = %event.category,
        component = %event.component,
        "logging kaizen event"
    );
    sqlx::query(
        r#"
        INSERT INTO kaizen_events (
            id, created_at, request_id, category, severity, component, message, metadata_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&id)
    .bind(&created_at)
    .bind(&event.request_id)
    .bind(&event.category)
    .bind(severity_label(&event.severity))
    .bind(&event.component)
    .bind(&event.message)
    .bind(&metadata_json)
    .execute(&state.pool)
    .await?;
    Ok(())
}
impl TryFrom<RegistryRow> for RegistryRecord {
    type Error = AppError;
    fn try_from(value: RegistryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            namespace: value.namespace,
            kind: value.kind,
            key: value.key,
            version: value.version,
            value: serde_json::from_str(&value.value_json).map_err(|error| {
                AppError::Internal(format!("failed to deserialize registry value: {error}"))
            })?,
            provenance: serde_json::from_str(&value.provenance_json).map_err(|error| {
                AppError::Internal(format!(
                    "failed to deserialize registry provenance: {error}"
                ))
            })?,
            digest_sha256: value.digest_sha256,
            signature_ed25519: value.signature_ed25519,
            created_at: value.created_at,
            updated_at: value.updated_at,
            retired_at: value.retired_at,
            tags: serde_json::from_str(&value.tags_json).map_err(|error| {
                AppError::Internal(format!("failed to deserialize registry tags: {error}"))
            })?,
        })
    }
}
fn validate_registry_input(namespace: &str, kind: &str, key: &str) -> Result<(), AppError> {
    for (label, value) in [("namespace", namespace), ("kind", kind), ("key", key)] {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidInput(format!("{label} must not be empty")));
        }
        if trimmed.len() > 128 {
            return Err(AppError::InvalidInput(format!(
                "{label} exceeds 128 characters"
            )));
        }
    }
    Ok(())
}
fn registry_digest(
    namespace: &str,
    kind: &str,
    key: &str,
    version: i64,
    value_json: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update([0]);
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(key.as_bytes());
    hasher.update([0]);
    hasher.update(version.to_string().as_bytes());
    hasher.update([0]);
    hasher.update(value_json.as_bytes());
    bytes_to_hex(&hasher.finalize())
}
fn sign_digest(signing_key: Option<&SigningKey>, digest: &str) -> Option<String> {
    let signing_key = signing_key?;
    let signature = signing_key.sign(digest.as_bytes());
    Some(bytes_to_hex(&signature.to_bytes()))
}
fn parse_signing_key(raw_hex: &str) -> anyhow::Result<SigningKey> {
    let bytes = decode_hex(raw_hex).context("failed to decode TIE_SIGNING_KEY_HEX as hex")?;
    let secret: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("TIE_SIGNING_KEY_HEX must contain exactly 32 bytes"))?;
    Ok(SigningKey::from_bytes(&secret))
}
fn decode_hex(input: &str) -> anyhow::Result<Vec<u8>> {
    let normalized = input.trim();
    if normalized.len() % 2 != 0 {
        anyhow::bail!("hex string must contain an even number of characters");
    }
    let mut out = Vec::with_capacity(normalized.len() / 2);
    let bytes = normalized.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let high = hex_nibble(bytes[idx])?;
        let low = hex_nibble(bytes[idx + 1])?;
        out.push((high << 4) | low);
        idx += 2;
    }
    Ok(out)
}
fn hex_nibble(byte: u8) -> anyhow::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        other => anyhow::bail!("invalid hex character '{}'", other as char),
    }
}
fn bytes_to_hex(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(LUT[(byte >> 4) as usize] as char);
        out.push(LUT[(byte & 0x0f) as usize] as char);
    }
    out
}
fn validation_cache_key(request: &ValidationRequest) -> Result<String, AppError> {
    let mut hasher = AHasher::default();
    request.hash(&mut hasher);
    let hash = hasher.finish();
    let bytes = hash.to_be_bytes();
    Ok(bytes_to_hex(&bytes))
}
fn new_request_id() -> String {
    Uuid::now_v7().to_string()
}
fn default_empty_object() -> Value {
    json!({})
}
fn verdict_label(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Warn => "warn",
        Verdict::Fail => "fail",
        Verdict::Inconclusive => "inconclusive",
    }
}
fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Critical => "critical",
    }
}
impl Hash for SubjectType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            SubjectType::Code => 0u8.hash(state),
            SubjectType::Fact => 1u8.hash(state),
            SubjectType::Action => 2u8.hash(state),
        }
    }
}
impl Hash for ValidationRequest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.request_id.hash(state);
        self.subject_type.hash(state);
        self.subject.hash(state);
        self.registry_record_ids.hash(state);
        if !self.metadata.is_null() && self.metadata.as_object().map_or(true, |o| !o.is_empty()) {
            if let Ok(meta_str) = serde_json::to_string(&self.metadata) {
                meta_str.hash(state);
            }
        }
    }
}
