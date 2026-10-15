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
/// "YYYY-MM-DD" (UTC) for Unix seconds.
pub fn civil_date(secs: u64) -> String {
    // Howard Hinnant's days → civil date.
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Construct an extensible serialized record without exhaustive field literals.
pub fn record<T: Default>(configure: impl FnOnce(&mut T)) -> T {
    let mut value = T::default();
    configure(&mut value);
    value
}
