import re

with open("tie/src/main.rs", "r") as f:
    content = f.read()

# 1. Add std::hash::{Hash, Hasher} and seahash
# Let's insert them near the other imports
content = content.replace("use sha2::{Digest, Sha256};", "use sha2::{Digest, Sha256};\nuse std::hash::{Hash, Hasher};\nuse seahash::SeaHasher;")

# 2. Add Hash derive to SubjectType
content = content.replace("#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]\n#[serde(rename_all = \"snake_case\")]\n#[cfg_attr(feature = \"swagger-ui\", derive(ToSchema))]\nenum SubjectType", "#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]\n#[serde(rename_all = \"snake_case\")]\n#[cfg_attr(feature = \"swagger-ui\", derive(ToSchema))]\nenum SubjectType")

# 3. Replace validation_cache_key implementation
old_fn = """fn validation_cache_key(request: &ValidationRequest) -> Result<String, AppError> {
    let serialized = serde_json::to_string(request)
        .map_err(|error| AppError::Internal(format!("failed to serialize request: {error}")))?;
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    Ok(bytes_to_hex(&hasher.finalize()))
}"""

new_fn = """fn validation_cache_key(request: &ValidationRequest) -> Result<String, AppError> {
    let mut hasher = SeaHasher::new();
    request.subject_type.hash(&mut hasher);
    request.subject.hash(&mut hasher);
    for id in &request.registry_record_ids {
        id.hash(&mut hasher);
    }

    // We explicitly DO NOT hash request_id (unique per request)
    // or metadata (unstructured and irrelevant for validation identity).

    Ok(format!("{:016x}", hasher.finish()))
}"""

content = content.replace(old_fn, new_fn)

with open("tie/src/main.rs", "w") as f:
    f.write(content)

print("Patch applied")
