use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use theory_engine::{providers::*, storage::data_directory};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let provider = if args.get(1).is_some_and(|s| s == "claude") {
        ProviderKind::Claude
    } else {
        ProviderKind::Codex
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let adapter = Subscriptions::new(&data_directory(), Settings::default());
    let c = adapter.inspect(provider, &cancel)?;
    println!("{}", serde_json::to_string_pretty(&c)?);
    if args.iter().any(|s| s == "--generate") {
        let request = Request::new(
            "Return the JSON object with status = ok. This is a synthetic connection test.".into(),
            serde_json::json!({"type":"object","properties":{"status":{"type":"string"}},"required":["status"],"additionalProperties":false}),
            String::new(),
            Instant::now() + Duration::from_secs(90),
        );
        let r = adapter.generate(provider, &request, &cancel, &mut |s| eprintln!("{s}"))?;
        println!("model={} result={}", r.model, r.value);
    }
    Ok(())
}
