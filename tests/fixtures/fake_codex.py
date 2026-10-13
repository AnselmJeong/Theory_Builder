#!/usr/bin/python3
"""Deterministic transport fixture. Never contacts a provider."""
import json
import pathlib
import sys
import time
root = pathlib.Path(__file__).parent
scenario = (root / "scenario").read_text()
def emit(value):
    print(json.dumps(value, ensure_ascii=False), flush=True)
for line in sys.stdin:
    request = json.loads(line)
    with (root / "requests.jsonl").open("a") as f:
        f.write(json.dumps(request) + "\n")
    if "id" not in request or "method" not in request:
        continue
    method = request["method"]
    if scenario == "silent":
        time.sleep(60)
    result = {}
    if method == "account/read":
        result = {"account": {"type": "apiKey" if scenario == "api" else "chatgpt"}}
    elif method == "model/list":
        result = {"data": [{"model": "fixture-model", "id": "display-id", "hidden": False, "isDefault": True, "defaultReasoningEffort": "low"}], "nextCursor": None}
    elif method == "account/rateLimits/read":
        result = {"ordinaryUsageAllowed": scenario != "denied", "rateLimits": {"limitId": "codex", "primary": {"usedPercent": 100 if scenario == "exhausted" else 12}}}
    elif method == "thread/start":
        result = {"thread": {"id": "thread-1"}}
    elif method == "turn/start":
        if scenario == "oversized":
            print("x" * (4 * 1024 * 1024 + 1), flush=True)
            continue
        if scenario == "malformed":
            print("not json", flush=True)
            continue
        emit({"method": "turn/completed", "params": {"threadId": "unrelated", "turn": {"status": "completed"}}})
        emit({"id": "server-tool", "method": "item/commandExecution/requestApproval", "params": {}})
        rejection = json.loads(sys.stdin.readline())
        with (root / "requests.jsonl").open("a") as f:
            f.write(json.dumps(rejection) + "\n")
        emit({"method": "item/agentMessage/delta", "params": {"threadId": "thread-1", "turnId": "turn-1", "itemId": "answer", "delta": '{"status":"한글 ok"}'}})
        emit({"method": "item/completed", "params": {"threadId": "thread-1", "turnId": "turn-1", "item": {"id": "answer", "type": "agentMessage", "text": '{"status":"한글 ok"}'}}})
        result = {"turn": {"id": "turn-1"}}
        emit({"id": request["id"], "result": result})
        emit({"method": "turn/completed", "params": {"threadId": "thread-1", "turnId": "turn-1", "turn": {"id": "turn-1", "status": "failed" if scenario == "failed" else "completed", "error": "fixture failure"}}})
        continue
    emit({"id": request["id"], "result": result})
