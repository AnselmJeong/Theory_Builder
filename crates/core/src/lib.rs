//! Pure research contracts and validation. No UI, database, or provider dependencies.
pub mod bib;
pub mod planning;
pub mod report;
pub mod research;
pub use bib::*;
pub use planning::*;
pub use report::*;
pub use research::*;

pub fn id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}
pub fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Construct an extensible serialized record without exhaustive field literals.
pub fn record<T: Default>(configure: impl FnOnce(&mut T)) -> T {
    let mut value = T::default();
    configure(&mut value);
    value
}
