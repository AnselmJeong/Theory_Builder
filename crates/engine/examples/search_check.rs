use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
fn main() -> anyhow::Result<()> {
    let query = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "melatonin anxiety receptor mechanism".into());
    let result = theory_engine::search::retrieve(
        &[query],
        &Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(180),
        &mut |s| eprintln!("{s}"),
    )?;
    println!(
        "sources={} warnings={:?}",
        result.sources.len(),
        result.warnings
    );
    for s in &result.sources {
        println!(
            "{:?} · {} lines · {} · {}",
            s.access,
            s.text.lines().count(),
            s.title,
            s.locator
        );
    }
    anyhow::ensure!(
        result.sources.iter().any(|s| !s.text.is_empty()),
        "no readable source"
    );
    Ok(())
}
