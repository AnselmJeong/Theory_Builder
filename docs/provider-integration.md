# Subscription integration

Theory Studio is a local Rust/GPUI application. All planner, specialist, critic and synthesis calls use one subscription provider interface; no HTTP API billing implementation exists. Codex App Server 0.160.0 and Claude Code 2.1.296 are the inspected CLI versions.

Codex uses a private Application Support/Theory Studio/providers/codex auth home (file credential storage) and empty scratch directory. OAuth stays in Codex. Account-specific models and included allowance are refreshed before turns. Unknown/denied ordinary usage blocks generation. Standard service tier only. There is no purchase/reset or API fallback path. A quota preflight is not an atomic billing reservation.

Claude uses the official installed Claude Code CLI and its own existing subscription login via `auth status`; API-key accounts are rejected. An environment allowlist excludes API keys, provider endpoints and injected runtime settings. `--safe-mode --tools '' --strict-mcp-config --mcp-config '{"mcpServers":{}}' --setting-sources ''` isolates research calls from customization/tools. No tokens are copied. Claude CLI does not expose a reliable remaining-subscription quota in auth status; the UI says unknown and propagates quota errors without fallback. Claude account extra-usage settings remain account-owned.

One worker owns each run; model calls are serial initially. Fresh contexts expose only pinned source revisions and explicit dependency outputs. Cancellation stops reservations, interrupts Codex, and terminates the owned process group; remote billing/completion after cancellation cannot be guaranteed. Credentials and hidden reasoning are not logged. Selected source passages, actual request context and public output are recorded locally for auditing.

Delivery: source, automated tests, macOS bundle and local launch. Real OAuth/inference validation is separate from fixture tests and must be reported accurately.

Literature retrieval is separate from model-provider tools. With search enabled (default), the planner may request up to two focused queries. The engine queries PubMed and the existing TinyFish Search/Fetch endpoints using ScholarPen configuration. Credentials remain engine-only. Search results without fetched text are metadata only; PubMed abstracts and fetched web excerpts retain their actual access scope, URL, retrieval time and source hash. Sources are persisted before analysis and citations must match the text and line range actually supplied.
