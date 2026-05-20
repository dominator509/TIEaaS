use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    Code,
    Fact,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    pub request_id: String,
    pub subject_type: SubjectType,
    pub subject: String,
    pub registry_record_ids: Vec<String>,
    pub metadata: Value,
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

fn validation_cache_key_old(request: &ValidationRequest) -> String {
    let serialized = serde_json::to_string(request).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

fn validation_cache_key_new(request: &ValidationRequest) -> String {
    let mut hasher = seahash::SeaHasher::new();
    request.subject_type.hash(&mut hasher);
    request.subject.hash(&mut hasher);
    for id in &request.registry_record_ids {
        id.hash(&mut hasher);
    }
    // Note: We deliberately exclude request_id and metadata from the cache key!
    // request_id is unique per request and defeats the cache.
    // metadata is unstructured and should not affect the core validation subject.

    // Hash metadata string if needed, or exclude it completely if not part of identity
    // let metadata_str = serde_json::to_string(&request.metadata).unwrap();
    // metadata_str.hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}

fn bench_cache_key(c: &mut Criterion) {
    let request = ValidationRequest {
        request_id: "018f9b9f-9b1a-7b3b-8b1a-9b3b9b1a7b3b".to_string(),
        subject_type: SubjectType::Fact,
        subject: "{\"claim\": \"Rust provides memory safety without a garbage collector.\"}"
            .to_string(),
        registry_record_ids: vec!["rec_1".to_string(), "rec_2".to_string()],
        metadata: json!({"critical": false, "source": "benchmark", "complex": {"nested": true}}),
    };

    let mut group = c.benchmark_group("cache_key");
    group.bench_function("old_json_sha256", |b| {
        b.iter(|| validation_cache_key_old(black_box(&request)))
    });
    group.bench_function("new_seahash", |b| {
        b.iter(|| validation_cache_key_new(black_box(&request)))
    });
    group.finish();
}

criterion_group!(benches, bench_cache_key);
criterion_main!(benches);
