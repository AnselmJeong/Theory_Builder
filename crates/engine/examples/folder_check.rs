use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use theory_engine::{providers::Cancel, storage::*};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let root = PathBuf::from(&args[1]);
    let folder = PathBuf::from(&args[2]);
    let mut store = Store::open_folder(&root, &folder)?;
    let cancel: Cancel = Arc::new(AtomicBool::new(false));
    store.sync_resources(&cancel, &mut |s| eprintln!("{s}"))?;
    for s in &store.project.sources {
        println!(
            "{}: {:?}, {} lines, {}",
            s.title,
            s.access,
            s.text.lines().count(),
            s.import_note
        );
    }
    anyhow::ensure!(
        store
            .project
            .sources
            .iter()
            .all(|s| !s.text.trim().is_empty()),
        "unread source"
    );
    let key = store.project.selection_key();
    let before = store.project.sources.len();
    drop(store);
    let mut store = Store::open(&root, &key)?;
    anyhow::ensure!(
        !store.sync_resources(&cancel, &mut |_| {})?,
        "unchanged folder changed"
    );
    anyhow::ensure!(store.project.sources.len() == before, "duplicate sources");
    println!("reopen + unchanged sync passed");
    Ok(())
}
