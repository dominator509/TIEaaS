use crate::app_state::AppState;
use crate::error::AppError;
use crate::models::{RegistryRecord, RegistryRecordUpdate, RegistryRecordUpsert, RegistryRow};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use ed25519_dalek::{Signer, Verifier, Signature, SigningKey, VerifyingKey};

pub async fn create_record(
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

    let signature = if let Some(key) = &state.signing_key {
        let sig = key.sign(digest.as_bytes());
        Some(crate::utils::bytes_to_hex(&sig.to_bytes()))
    } else {
        None
    };

    sqlx::query(
        r#"
        INSERT INTO registry_records (
            id, namespace, kind, key, version, value_json, provenance_json,
            digest_sha256, signature_ed25519, created_at, updated_at, tags_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
    .bind(&signature)
    .bind(&now)
    .bind(&now)
    .bind(&tags_json)
    .execute(&state.pool)
    .await?;

    get_record_by_id(state, &id).await
}

pub async fn list_records(
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

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        records.push(row.try_into()?);
    }

    Ok(records)
}

pub async fn get_record_by_id(state: &AppState, id: &str) -> Result<RegistryRecord, AppError> {
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

    // Verify signature if present and system has a key
    if let (Some(sig_hex), Some(key)) = (&record.signature_ed25519, &state.signing_key) {
        verify_registry_signature(key, &record.digest_sha256, sig_hex)?;
    }

    state
        .registry_cache
        .insert(id.to_string(), record.clone())
        .await;
    Ok(record)
}

pub async fn get_latest_record_by_key(
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

    // Verify signature if present and system has a key
    if let (Some(sig_hex), Some(key)) = (&record.signature_ed25519, &state.signing_key) {
        verify_registry_signature(key, &record.digest_sha256, sig_hex)?;
    }

    state
        .registry_cache
        .insert(record.id.clone(), record.clone())
        .await;
    Ok(record)
}

pub async fn supersede_record(
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

pub async fn soft_delete_record(state: &AppState, id: &str) -> Result<(), AppError> {
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

pub fn verify_registry_signature(
    signing_key: &SigningKey,
    digest_hex: &str,
    signature_hex: &str,
) -> Result<(), AppError> {
    let sig_bytes = crate::utils::decode_hex(signature_hex).map_err(|e| {
        AppError::InvalidInput(format!("Invalid signature hex encoding: {e}"))
    })?;

    let sig = Signature::from_slice(&sig_bytes).map_err(|e| {
        AppError::InvalidInput(format!("Invalid Ed25519 signature format: {e}"))
    })?;

    let verifying_key = VerifyingKey::from(signing_key);
    verifying_key.verify(digest_hex.as_bytes(), &sig).map_err(|e| {
        AppError::InvalidInput(format!("Registry record signature verification failed: {e}"))
    })?;

    Ok(())
}

fn validate_registry_input(namespace: &str, kind: &str, key: &str) -> Result<(), AppError> {
    for (label, value) in [("namespace", namespace), ("kind", kind), ("key", key)] {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidInput(format!("{label} must not be empty ")));
        }
        if trimmed.len() > 128 {
            return Err(AppError::InvalidInput(format!(
                "{label} exceeds 128 characters "
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
    crate::utils::bytes_to_hex(&hasher.finalize())
}
