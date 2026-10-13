use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use theory_core::Budget;
use theory_engine::{
    execution::{RunInput, run},
    providers::*,
    storage::*,
};
fn main() -> anyhow::Result<()> {
    let auth_root = data_directory();
    let args: Vec<_> = std::env::args().collect();
    let output = PathBuf::from(args.get(2).expect("provider output-folder [question-file]"));
    let question = std::fs::read_to_string(args.get(3).expect("question-file"))?;
    let mut settings: Settings =
        serde_json::from_slice(&std::fs::read(auth_root.join("settings.json"))?)?;
    settings.primary = if args[1] == "claude" {
        ProviderKind::Claude
    } else {
        ProviderKind::Codex
    };
    settings.cross_review = false;
    let mut store = Store::create(&output, "실제 provider 전체 계약 검증")?;
    let adapter = Subscriptions::new(&auth_root, settings.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    run(
        &mut store,
        RunInput::new(question, Budget::Standard),
        &settings,
        &adapter,
        &cancel,
        &mut |_, s| eprintln!("{s}"),
    )?;
    let r = store.project.runs.last().unwrap();
    println!(
        "state={:?} calls={} tasks={:?}",
        r.state,
        r.calls,
        r.attempts
            .iter()
            .map(|a| (&a.task_id, a.state, &a.error))
            .collect::<Vec<_>>()
    );
    anyhow::ensure!(
        r.attempts
            .iter()
            .all(|a| a.state == theory_core::TaskState::Succeeded),
        "실제 작업 실패"
    );
    println!("export={}", store.export(&output)?.display());
    Ok(())
}
