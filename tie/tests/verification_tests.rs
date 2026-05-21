#[allow(dead_code)]
mod tie_impl {
    #![allow(clippy::all)]
    include!("../src/main.rs");

    #[cfg(test)]
    mod tests {
        use super::*;
        use ed25519_dalek::SigningKey;
        use serde_json::json;
        use sqlx::sqlite::SqlitePoolOptions;
        use std::{sync::Arc, time::Duration};

        async fn test_state(
            require_fact_citations: bool,
            require_action_approval: bool,
        ) -> AppState {
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
                    require_fact_citations,
                    require_action_approval,
                },
                signing_key: Some(Arc::new(SigningKey::from_bytes(&[7_u8; 32]))),
            }
        }

        #[tokio::test]
        async fn code_verifier_flags_dangerous_patterns() {
            let request = ValidationRequest {
                request_id: "req-code-danger".to_string(),
                subject_type: SubjectType::Code,
                subject: "unsafe fn wipe() { std::process::Command::new(\"rm\"); }".to_string(),
                registry_record_ids: Vec::new(),
                metadata: json!({}),
            };

            let evidence = run_code_verifier(&request, &[])
                .await
                .expect("code verifier should return evidence for code requests");

            assert_eq!(evidence.verdict, Verdict::Fail);
            assert_eq!(evidence.severity, Severity::Critical);
            assert!(evidence.message.contains("dangerous code pattern"));
            assert!(evidence.score < 0.1);
        }

        #[tokio::test]
        async fn fact_verifier_requires_citations_when_configured() {
            let request = ValidationRequest {
                request_id: "req-fact-no-citation".to_string(),
                subject_type: SubjectType::Fact,
                subject: "This claim is definitely correct.".to_string(),
                registry_record_ids: Vec::new(),
                metadata: json!({}),
            };

            let evidence = run_fact_verifier(&request, &[], true)
                .await
                .expect("fact verifier should return evidence for fact requests");

            assert_eq!(evidence.verdict, Verdict::Fail);
            assert_eq!(evidence.severity, Severity::Error);
            assert!(evidence.message.contains("requires citations"));
        }

        #[tokio::test]
        async fn action_verifier_blocks_high_risk_actions_and_warns_on_missing_approval() {
            let risky = ValidationRequest {
                request_id: "req-action-risky".to_string(),
                subject_type: SubjectType::Action,
                subject: "Delete production database snapshot and disable auth".to_string(),
                registry_record_ids: Vec::new(),
                metadata: json!({}),
            };

            let risky_ev = run_action_verifier(&risky, &[], true)
                .await
                .expect("action verifier should return evidence for action requests");
            assert_eq!(risky_ev.verdict, Verdict::Fail);
            assert_eq!(risky_ev.severity, Severity::Critical);

            let missing_approval = ValidationRequest {
                request_id: "req-action-approval".to_string(),
                subject_type: SubjectType::Action,
                subject: "Restart service after routine maintenance".to_string(),
                registry_record_ids: Vec::new(),
                metadata: json!({}),
            };
            let warning_ev = run_action_verifier(&missing_approval, &[], true)
                .await
                .expect("action verifier should warn when approval is required");

            assert_eq!(warning_ev.verdict, Verdict::Warn);
            assert_eq!(warning_ev.severity, Severity::Warning);
            assert!(warning_ev.message.contains("approval_token"));
        }

        #[tokio::test]
        async fn registry_records_increment_versions_and_emit_signatures() {
            let state = test_state(true, true).await;

            let first = create_record(
                &state,
                RegistryRecordUpsert {
                    namespace: "specs".to_string(),
                    kind: "fact".to_string(),
                    key: "moon-composition".to_string(),
                    value: json!({"claim": "The moon is rocky"}),
                    provenance: json!({"source": "nasa"}),
                    tags: vec!["astronomy".to_string()],
                },
            )
            .await
            .expect("first registry record");

            let second = create_record(
                &state,
                RegistryRecordUpsert {
                    namespace: "specs".to_string(),
                    kind: "fact".to_string(),
                    key: "moon-composition".to_string(),
                    value: json!({"claim": "The moon is silicate-rich"}),
                    provenance: json!({"source": "nasa", "reviewed": true}),
                    tags: vec!["astronomy".to_string(), "reviewed".to_string()],
                },
            )
            .await
            .expect("second registry record");

            assert_eq!(first.version, 1);
            assert_eq!(second.version, 2);
            assert_ne!(first.id, second.id);
            assert!(first.signature_ed25519.is_some());
            assert!(second.signature_ed25519.is_some());
            assert_ne!(first.digest_sha256, second.digest_sha256);

            let latest = get_latest_record_by_key(&state, "specs", "fact", "moon-composition")
                .await
                .expect("latest active record");
            assert_eq!(latest.version, 2);
        }

        #[test]
        fn policy_modes_resolve_verdicts_as_expected() {
            let warn_item = EvidenceItem {
                adapter: "fact_verifier".to_string(),
                verdict: Verdict::Warn,
                severity: Severity::Warning,
                message: "review this".to_string(),
                score: 0.6,
                references: Vec::new(),
                duration_ms: 5,
            };
            let critical_fail_item = EvidenceItem {
                adapter: "code_verifier".to_string(),
                verdict: Verdict::Fail,
                severity: Severity::Critical,
                message: "critical".to_string(),
                score: 0.0,
                references: Vec::new(),
                duration_ms: 7,
            };

            assert_eq!(
                resolve_verdict(PolicyMode::Advisory, &[warn_item.clone()]),
                Verdict::Warn
            );
            assert_eq!(
                resolve_verdict(PolicyMode::CriticalFailClosed, &[warn_item.clone()]),
                Verdict::Warn
            );
            assert_eq!(
                resolve_verdict(
                    PolicyMode::CriticalFailClosed,
                    &[critical_fail_item.clone()]
                ),
                Verdict::Fail
            );
            assert_eq!(
                resolve_verdict(PolicyMode::FullFailClosed, &[warn_item]),
                Verdict::Warn
            );
            assert_eq!(
                resolve_verdict(PolicyMode::FullFailClosed, &[critical_fail_item]),
                Verdict::Fail
            );
        }
    }
}
