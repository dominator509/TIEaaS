use ahash::AHasher;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::hash::{Hash, Hasher};

// Existing types (mocked for benchmarking)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SubjectType {
    Code,
    Fact,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationRequest {
    request_id: String,
    subject_type: SubjectType,
    subject: String,
    registry_record_ids: Vec<String>,
    metadata: Value,
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

// Current implementation
fn current_validation_cache_key(request: &ValidationRequest) -> String {
    let serialized = serde_json::to_string(request).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    bytes_to_hex(&hasher.finalize())
}

// Proposed implementation
// We need to implement Hash manually because Value doesn't implement Hash.
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
        // Serialize Value to string for hashing
        // In practice this is still slightly expensive, but much cheaper than serializing the whole struct
        if !self.metadata.is_null() && self.metadata.as_object().map_or(true, |o| !o.is_empty()) {
            let meta_str = serde_json::to_string(&self.metadata).unwrap_or_default();
            meta_str.hash(state);
        }
    }
}

fn proposed_validation_cache_key(request: &ValidationRequest) -> String {
    let mut hasher = AHasher::default();
    request.hash(&mut hasher);
    let hash = hasher.finish();
    // Convert 64-bit hash to hex string (16 chars)
    let bytes = hash.to_be_bytes();
    bytes_to_hex(&bytes)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let request = ValidationRequest {
        request_id: "req-12345".to_string(),
        subject_type: SubjectType::Fact,
        subject: "{\"claim\": \"Rust is fast\"}".to_string(),
        registry_record_ids: vec!["rec-1".to_string(), "rec-2".to_string()],
        metadata: serde_json::json!({"source": "benchmark", "critical": false}),
    };

    let mut group = c.benchmark_group("Validation Cache Key");

    group.bench_function("current_json_sha256", |b| {
        b.iter(|| current_validation_cache_key(black_box(&request)))
    });

    group.bench_function("proposed_ahash_hash", |b| {
        b.iter(|| proposed_validation_cache_key(black_box(&request)))
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
