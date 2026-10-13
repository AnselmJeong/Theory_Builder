use super::{Connection, Request, Response, Settings, process::*};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn command(root: &Path, settings: &Settings) -> Result<std::process::Command> {
    let cwd = root.join("providers/claude-scratch");
    private_dir(&cwd)?;
    let exe = executable("claude", &settings.claude_path)?;
    Ok(isolated_command(&exe, &cwd))
}
fn status(root: &Path, s: &Settings, cancel: &Cancel) -> Result<Value> {
    let mut cmd = command(root, s)?;
    cmd.args(["--safe-mode", "auth", "status", "--json"]);
    Ok(serde_json::from_str(&Process::text(
        cmd,
        cancel,
        Instant::now() + Duration::from_secs(25),
        None,
    )?)?)
}
fn subscription(v: &Value) -> bool {
    v["loggedIn"] == true
        && v["authMethod"]
            .as_str()
            .is_some_and(|s| s == "claude.ai" || s == "oauth")
        && v["subscriptionType"]
            .as_str()
            .is_some_and(|s| ["pro", "max", "team", "enterprise"].contains(&s))
}
pub fn inspect(root: &Path, s: &Settings, cancel: &Cancel) -> Result<Connection> {
    let exe = executable("claude", &s.claude_path)?;
    let v = status(root, s, cancel)?;
    let ok = subscription(&v);
    Ok(Connection {
        provider: "Claude".into(),
        installed: true,
        authenticated: ok,
        message: if ok {
            "Claude Code 구독 인증 완료 · 모델 별칭은 계정에서 허용된 경우 사용할 수 있습니다"
        } else {
            "Claude Code 구독 로그인이 필요합니다. API 인증은 사용할 수 없습니다."
        }
        .into(),
        models: if ok {
            vec!["sonnet".into(), "opus".into(), "haiku".into()]
        } else {
            vec![]
        },
        allowance: None,
        executable: exe.display().to_string(),
    })
}
pub fn login(
    root: &Path,
    s: &Settings,
    cancel: &Cancel,
    progress: &mut dyn FnMut(String),
) -> Result<()> {
    progress(
        "브라우저에서 Claude Code 구독 로그인을 완료하세요. 기존 Claude Code 계정에도 적용됩니다."
            .into(),
    );
    let mut cmd = command(root, s)?;
    cmd.args(["auth", "login", "--claudeai"]);
    Process::text(cmd, cancel, Instant::now() + Duration::from_secs(300), None)?;
    Ok(())
}
pub fn generate(
    root: &Path,
    s: &Settings,
    r: &Request,
    cancel: &Cancel,
    progress: &mut dyn FnMut(String),
) -> Result<Response> {
    ensure!(
        subscription(&status(root, s, cancel)?),
        "Claude Code 구독 로그인이 필요합니다. API 키로 전환하지 않았습니다."
    );
    let mut cmd = command(root, s)?;
    cmd.args([
        "--safe-mode",
        "--print",
        "--output-format",
        "json",
        "--tools",
        "",
        "--strict-mcp-config",
        "--mcp-config",
        "{\"mcpServers\":{}}",
        "--setting-sources",
        "",
        "--no-session-persistence",
        "--disable-slash-commands",
        "--permission-mode",
        "dontAsk",
        "--json-schema",
        &r.schema.to_string(),
        "--system-prompt",
        include_str!("../../../../resources/roles/research-v1.txt"),
    ]);
    if !r.model.is_empty() {
        cmd.args(["--model", &r.model]);
    }
    progress("Claude가 응답을 작성하고 있습니다".into());
    let raw = Process::text(cmd, cancel, r.deadline, Some(&r.prompt))?;
    let wire: Value = serde_json::from_str(&raw)?;
    let v = result_message(&wire)?;
    ensure!(
        v["is_error"] != true,
        "Claude 응답 실패: {}",
        v["result"]
            .as_str()
            .unwrap_or("사용량 또는 인증을 확인하세요")
    );
    let value = if v["structured_output"].is_object() {
        v["structured_output"].clone()
    } else {
        serde_json::from_str(v["result"].as_str().unwrap_or(""))?
    };
    let model = v["modelUsage"]
        .as_object()
        .and_then(|m| m.keys().next())
        .cloned()
        .unwrap_or_else(|| {
            if r.model.is_empty() {
                "account default (unreported)".into()
            } else {
                r.model.clone()
            }
        });
    Ok(Response {
        value,
        model,
        usage: Some(
            json!({"tokens":v.get("usage"),"model_usage":v.get("modelUsage"),"subscription_remaining":null}),
        ),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_api_accounts() {
        assert!(!subscription(
            &json!({"loggedIn":true,"authMethod":"api_key","subscriptionType":"max"})
        ));
        assert!(subscription(
            &json!({"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max"})
        ));
    }
}

fn result_message(wire: &Value) -> Result<&Value> {
    let result = if let Some(events) = wire.as_array() {
        events
            .iter()
            .rev()
            .find(|v| v["type"] == "result")
            .ok_or_else(|| anyhow::anyhow!("Claude CLI가 완료 결과를 반환하지 않았습니다"))?
    } else {
        wire
    };
    ensure!(result["type"] == "result", "Claude CLI 결과 형식 오류");
    Ok(result)
}
#[cfg(test)]
mod wire_tests {
    use super::*;
    #[test]
    fn accepts_single_and_event_array_results() {
        let object = json!({"type":"result","structured_output":{"status":"ok"}});
        assert!(result_message(&object).is_ok());
        let array = json!([{"type":"system"},object]);
        assert_eq!(
            result_message(&array).unwrap()["structured_output"]["status"],
            "ok"
        );
        assert!(result_message(&json!([])).is_err());
    }
}
