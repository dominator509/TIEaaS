#[allow(dead_code)]
mod tie_impl {
    #![allow(clippy::all)]
    include!("../src/main.rs");

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;
        use sqlx::{sqlite::SqlitePoolOptions, Row};
        use std::time::Duration;

        async fn test_state() -> AppState {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .expect("in-memory sqlite pool");

            bootstrap_schema(&pool).await.expect("schema bootstrap");

            AppState {
                pool,
                registry_cache: Cache::builder()
                    .time_to_live(Duration::from_secs(60))
                    .max_capacity(256)
                    .build(),
                validation_cache: Cache::builder()
                    .time_to_live(Duration::from_secs(60))
                    .max_capacity(256)
                    .build(),
                config: AppConfig {
                    http_bind: "127.0.0.1:0".to_string(),
                    grpc_bind: "127.0.0.1:0".to_string(),
                    enable_grpc: false,
                    policy_mode: PolicyMode::CriticalFailClosed,
                    verifier_budget: Duration::from_millis(100),
                    require_fact_citations: true,
                    require_action_approval: true,
                },
                signing_key: None,
            }
        }

        #[tokio::test]
        async fn kaizen_events_are_persisted_with_expected_shape() {
            let state = test_state().await;

            log_kaizen_event(
                &state,
                KaizenEvent {
                    request_id: "req-kaizen-1".to_string(),
                    category: "validation_outcome".to_string(),
                    severity: Severity::Error,
                    component: "decision_layer".to_string(),
                    message: "validation finished with warn".to_string(),
                    metadata: json!({"verdict": "warn", "subject_type": "fact"}),
                },
            )
            .await
            .expect("kaizen event should be logged");

            let row = sqlx::query(
                "SELECT request_id, category, severity, component, message, metadata_json FROM kaizen_events LIMIT 1"
            )
            .fetch_one(&state.pool)
            .await
            .expect("fetch logged event");

            assert_eq!(row.get::<String, _>("request_id"), "req-kaizen-1");
            assert_eq!(row.get::<String, _>("category"), "validation_outcome");
            assert_eq!(row.get::<String, _>("severity"), "error");
            assert_eq!(row.get::<String, _>("component"), "decision_layer");
            assert!(row.get::<String, _>("message").contains("warn"));
            assert!(row.get::<String, _>("metadata_json").contains("subject_type"));
        }

        #[tokio::test]
        async fn timed_out_adapters_downgrade_to_inconclusive() {
            let evidence = run_with_budget(
                Duration::from_millis(5),
                async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Some(EvidenceItem {
                        adapter: "slow_adapter".to_string(),
                        verdict: Verdict::Pass,
                        severity: Severity::Info,
                        message: "finished".to_string(),
                        score: 1.0,
                        references: Vec::new(),
                        duration_ms: 20,
                    })
                },
                "slow_adapter",
            )
            .await
            .expect("timeout should synthesize evidence");

            assert_eq!(evidence.adapter, "slow_adapter");
            assert_eq!(evidence.verdict, Verdict::Inconclusive);
            assert_eq!(evidence.severity, Severity::Error);
            assert!(evidence.message.contains("exceeded verifier budget"));
        }

        #[test]
        fn app_error_response_uses_canonical_retryability() {
            let invalid = AppError::InvalidInput("bad payload".to_string()).error_response();
            let timeout = AppError::Timeout("adapter budget exceeded".to_string()).error_response();
            let not_found = AppError::NotFound("missing record".to_string()).error_response();

            assert_eq!(invalid.status(), actix_web::http::StatusCode::BAD_REQUEST);
            assert_eq!(timeout.status(), actix_web::http::StatusCode::GATEWAY_TIMEOUT);
            assert_eq!(not_found.status(), actix_web::http::StatusCode::NOT_FOUND);
        }

        #[test]
        fn highest_severity_prefers_critical_then_error_then_warning() {
            let items = vec![
                EvidenceItem {
                    adapter: "fact".to_string(),
                    verdict: Verdict::Warn,
                    severity: Severity::Warning,
                    message: "warn".to_string(),
                    score: 0.7,
                    references: Vec::new(),
                    duration_ms: 1,
                },
                EvidenceItem {
                    adapter: "code".to_string(),
                    verdict: Verdict::Fail,
                    severity: Severity::Critical,
                    message: "critical".to_string(),
                    score: 0.0,
                    references: Vec::new(),
                    duration_ms: 1,
                },
            ];

            assert_eq!(highest_severity(&items), Severity::Critical);
            assert_eq!(highest_severity(&items[..1]), Severity::Warning);
            assert_eq!(highest_severity(&[]), Severity::Info);
        }
    }
}
