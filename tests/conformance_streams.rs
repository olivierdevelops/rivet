//! T-03 — streamed HTTP bodies and child output: SSE / JSONL / lines
//! iteration inside `with … as NAME`, emitted items, bounded buffering under
//! a slow consumer, early break and upstream disconnects.

#[path = "transport_support/mod.rs"]
mod support;

use rivet::internal::domain::Value;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::*;

fn policy(base: &str) -> String {
    format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["{base}"]}},{{"capability":"allow_exec","targets":["/usr/bin/printf"]}}]}}"#
    )
}

// vhco:test transports.exchange_http -- S14: SSE events expose event/id and JSON data; deltas are emitted and the final text returned
#[tokio::test]
async fn sse_chat_reply() {
    let (port, _) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!(
        "operation chat.reply\n    param prompt text required\n    output text\n    emits text\n    text = \"\"\n    first = null\n    with http post \"{base}/chat\" as events\n        body json {{prompt: prompt, stream: true}}\n        stream sse\n        decode json\n        for event in events\n            if first == null\n                first = event\n            end\n            text += event.data.delta\n            emit event.data.delta\n        end\n    end\n    return text\nend\n\noperation chat.first\n    output json\n    with http post \"{base}/chat\" as events\n        stream sse\n        for event in events\n            return event\n        end\n    end\n    return null\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(&src, tmp.path().to_str().unwrap(), &policy(&base));
    let sink = Arc::new(Collect::default());
    let c = rt
        .request(
            "chat.reply",
            Value::object([("prompt", Value::text("Hello"))]),
            Some(sink.clone()),
        )
        .await
        .unwrap();
    assert_eq!(c.result, Value::text("Hello!"));
    assert_eq!(
        *sink.items.lock().unwrap(),
        vec![Value::text("Hel"), Value::text("lo"), Value::text("!")]
    );
    let c = rt.request("chat.first", Value::Null, None).await.unwrap();
    assert_eq!(c.result.get("event"), Some(&Value::text("delta")));
    assert_eq!(c.result.get("id"), Some(&Value::text("1")));
    assert_eq!(
        c.result.get("data").unwrap().get("delta"),
        Some(&Value::text("Hel"))
    );
}

// vhco:test transports.exchange_http -- S15/S17: JSON lines stop at `break` (later lines never surface); text lines drop terminators and keep the final unterminated line
#[tokio::test]
async fn jsonl_break_and_lines() {
    let (port, _) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!(
        "operation gen.run\n    output json\n    emits text\n    with http post \"{base}/api/generate\" as chunks\n        body json {{model: \"fixture\", prompt: \"Hello\", stream: true}}\n        stream jsonl\n        decode json\n        for chunk in chunks\n            emit chunk.response\n            if chunk.done\n                break\n            end\n        end\n    end\n    return null\nend\n\noperation logs.tail\n    output json\n    emits text\n    with http get \"{base}/logs\" as lines\n        stream lines\n        for line in lines\n            emit line\n        end\n    end\n    return null\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(&src, tmp.path().to_str().unwrap(), &policy(&base));
    let sink = Arc::new(Collect::default());
    rt.request("gen.run", Value::Null, Some(sink.clone()))
        .await
        .unwrap();
    assert_eq!(
        *sink.items.lock().unwrap(),
        vec![Value::text("a"), Value::text("b"), Value::text("c")]
    );
    let sink = Arc::new(Collect::default());
    rt.request("logs.tail", Value::Null, Some(sink.clone()))
        .await
        .unwrap();
    assert_eq!(
        *sink.items.lock().unwrap(),
        vec![Value::text("one"), Value::text("two"), Value::text("three")]
    );
}

// vhco:test transports.exchange_http -- a slow consumer applies backpressure to an endless SSE source (bounded buffering), and break disposes the connection
#[tokio::test]
async fn slow_consumer_is_bounded() {
    let (port, stats) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!(
        "operation hose.take\n    output integer\n    emits json\n    n = 0\n    with http get \"{base}/firehose\" as events\n        stream sse\n        for event in events\n            emit n\n            n += 1\n            if n == 20\n                break\n            end\n        end\n    end\n    return n\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(&src, tmp.path().to_str().unwrap(), &policy(&base));
    let sink = Arc::new(Collect {
        delay_ms: 25,
        ..Collect::default()
    });
    let c = rt
        .request("hose.take", Value::Null, Some(sink.clone()))
        .await
        .unwrap();
    assert_eq!(c.result, Value::Int(20));
    // ~0.5 s of slow consumption: the producer is held to socket buffers, not unbounded memory.
    let written = stats.streamed_bytes.load(Ordering::SeqCst);
    assert!(
        written < 32 * 1024 * 1024,
        "producer wrote {written} bytes; the client must not buffer without bound"
    );
    // Break closed the response: the fixture's writer observes the disconnect.
    let mut closed = false;
    for _ in 0..50 {
        if stats.stream_closed.load(Ordering::SeqCst) == 1 {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        closed,
        "the upstream connection must be closed at scope exit"
    );
}

// vhco:test transports.exchange_http -- an upstream disconnect mid-body surfaces as a typed connection error after the items already delivered
#[tokio::test]
async fn disconnect_mid_stream() {
    let (port, _) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!(
        "operation cut.read\n    output json\n    emits text\n    with http get \"{base}/broken\" as lines\n        stream lines\n        for line in lines\n            emit line\n        end\n    end\n    return null\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(&src, tmp.path().to_str().unwrap(), &policy(&base));
    let sink = Arc::new(Collect::default());
    let e = rt
        .request("cut.read", Value::Null, Some(sink.clone()))
        .await
        .unwrap_err();
    assert_eq!(e.kind, rivet::internal::domain::ErrorKind::Connection);
    assert_eq!(*sink.items.lock().unwrap(), vec![Value::text("ab")]);
}

// vhco:test transports.run_process -- child stdout streamed as JSON lines (macOS); unsupported.sandbox_backend before spawning where no sandbox backend is verified
#[tokio::test]
async fn child_jsonl_stream() {
    let src = "operation p.run\n    output json\n    total = 0\n    with command \"/usr/bin/printf\" as p\n        args [\"{\\\"n\\\":1}\\n{\\\"n\\\":2}\\n\"]\n        stream stdout jsonl\n        for item in p.stdout\n            total += item.n\n        end\n    end\n    return total\nend\n";
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(
        src,
        tmp.path().to_str().unwrap(),
        &policy("http://127.0.0.1:1"),
    );
    let r = rt.request("p.run", Value::Null, None).await;
    if cfg!(target_os = "macos") {
        assert_eq!(r.unwrap().result, Value::Int(3));
    } else {
        // Linux (backend gated until kernel >= 6.12) and Windows (no backend) refuse a
        // sandboxed spawn under policy.json before starting it (ADR-0003).
        let e = r.unwrap_err();
        assert_eq!(e.code, "unsupported.sandbox_backend", "{e:?}");
        assert_eq!((e.http_status(), e.exit_code()), (501, 5));
    }
}

// vhco:test execution.request_operation -- INC-2026-0013: a rejected `--input-jsonl` line cancels the local request even when it is noticed before the session registers the request (stdin kept open; repeated to catch the race)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_input_line_always_cancels_the_local_request() {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        "operation echo.items\n    output integer\n    emits text\n    receives text\n    count = 0\n    for item in incoming\n        emit item\n        count = count + 1\n    end\n    return count\nend\n",
    )
    .unwrap();
    for round in 0..20 {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
            .current_dir(dir.path())
            .args([
                "--file",
                "app.rivet",
                "request",
                "echo.items",
                "--input-jsonl",
                "-",
                "--stream",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"{not json\n").await.unwrap();
        // stdin stays open: only the rejected line may end the request.
        let out = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output())
            .await
            .unwrap_or_else(|_| {
                panic!("round {round}: the rejected line did not cancel the request")
            })
            .unwrap();
        drop(stdin);
        assert_eq!(out.status.code(), Some(2), "round {round}");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("validation.input")
                || String::from_utf8_lossy(&out.stderr).contains("validation.input"),
            "round {round}"
        );
    }
}

// vhco:test execution.request_operation -- INC-2026-0013: when the operation ends on its own, `rivet request --input-jsonl -` exits at once even though stdin is still open (the CLI no longer waits for the blocking stdin reader on shutdown)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn input_jsonl_exits_when_the_operation_ends_without_eof() {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.rivet"),
        "operation echo.two\n    output integer\n    emits text\n    receives text\n    count = 0\n    for item in incoming\n        emit item\n        count = count + 1\n        if count == 2\n            break\n        end\n    end\n    return count\nend\n",
    )
    .unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rivet"))
        .current_dir(dir.path())
        .args([
            "--file",
            "app.rivet",
            "request",
            "echo.two",
            "--input-jsonl",
            "-",
            "--stream",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"\"a\"\n\"b\"\n").await.unwrap();
    // stdin is never closed: the finished operation alone must end the process.
    let out = tokio::time::timeout(Duration::from_secs(10), child.wait_with_output())
        .await
        .expect("the CLI waited for stdin EOF after the operation ended")
        .unwrap();
    drop(stdin);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"type\":\"result\"") && stdout.contains("\"data\":2"),
        "{stdout}"
    );
}
