//! T-05 — scoped resources: HTTP, TCP, Unix sockets, WebSocket and processes
//! against local fixtures; handles close on return/break/error; EOF, nonzero
//! exits, cleanup and the Stage C refusals.

#[path = "transport_support/mod.rs"]
mod support;

use rivet::domain::{ErrorKind, Value};
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::*;

fn op(body: &str) -> String {
    let indented: String = body.lines().map(|l| format!("    {l}\n")).collect();
    format!("operation t.run\n    output json\n{indented}end\n")
}

fn net_policy(targets: &[String]) -> String {
    let t: Vec<String> = targets.iter().map(|s| format!("\"{s}\"")).collect();
    format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":[{}]}}]}}"#,
        t.join(",")
    )
}

async fn run(src: &str, policy: &str) -> Result<Value, rivet::domain::RivetError> {
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(src, tmp.path().to_str().unwrap(), policy);
    rt.request("t.run", Value::Null, None)
        .await
        .map(|c| c.result)
}

// vhco:test transports.exchange_http -- GET/POST/DELETE against a local fixture return {status, headers, body}; query values are encoded as data
#[tokio::test]
async fn http_one_shot_methods() {
    let (port, stats) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let policy = net_policy(std::slice::from_ref(&base));
    let v = run(
        &op(&format!(
            "r = http get \"{base}/users/42\"\n    decode json\nend\nreturn r"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v.get("status"), Some(&Value::Int(200)));
    assert_eq!(
        v.get("body").unwrap().get("name"),
        Some(&Value::text("Ada"))
    );
    assert_eq!(
        v.get("headers").unwrap().get("content-type"),
        Some(&Value::text("application/json"))
    );

    let v = run(
        &op(&format!(
            "r = http get \"{base}/search\"\n    query q \"rust & capy\"\n    query limit 10\n    decode json\nend\nreturn r.body"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(
        v,
        Value::List(vec![
            Value::object([
                ("key", Value::text("q")),
                ("value", Value::text("rust & capy"))
            ]),
            Value::object([("key", Value::text("limit")), ("value", Value::text("10"))]),
        ])
    );

    let v = run(
        &op(&format!(
            "r = http post \"{base}/users\"\n    body json {{name: \"Ada \\\"q\\\"\"}}\n    decode json\nend\nreturn r.body"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v.get("name"), Some(&Value::text("Ada \"q\"")));

    let v = run(
        &op(&format!(
            "r = http post \"{base}/form\"\n    body form {{username: \"ada\", password: \"a&b\"}}\n    decode json\nend\nreturn r.body"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(
        v.get("ct"),
        Some(&Value::text("application/x-www-form-urlencoded"))
    );
    assert_eq!(
        v.get("body"),
        Some(&Value::text("username=ada&password=a%26b"))
    );

    let v = run(
        &op(&format!(
            "r = http delete \"{base}/users/42\"\n    accept status [204]\n    decode bytes\nend\nreturn {{status: r.status, bytes: (length r.body)}}"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(
        v,
        Value::object([("status", Value::Int(204)), ("bytes", Value::Int(0))])
    );
    assert!(stats.requests.lock().unwrap().len() >= 5);
}

// vhco:test transports.exchange_http -- an interpolated `../admin?x=` stays one encoded path segment
#[tokio::test]
async fn http_interpolation_keeps_segments() {
    let (port, stats) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = format!(
        "operation t.run\n    param id text required\n    output json\n    r = http get \"{base}/users/${{id}}\"\n        decode json\n    end\n    return r.body\nend\n"
    );
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(&src, tmp.path().to_str().unwrap(), &net_policy(&[base]));
    let v = rt
        .request(
            "t.run",
            Value::object([("id", Value::text("../admin?x="))]),
            None,
        )
        .await
        .unwrap()
        .result;
    assert_eq!(
        v.get("path"),
        Some(&Value::text("/users/..%2Fadmin%3Fx%3D"))
    );
    assert_eq!(
        stats.requests.lock().unwrap().as_slice(),
        ["GET /users/..%2Fadmin%3Fx%3D".to_string()]
    );
    let e = rt
        .request("t.run", Value::object([("id", Value::text(".."))]), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "validation.url_segment");
}

// vhco:test transports.exchange_http -- non-2xx fails http.status with details.status unless accepted; retries replay GET; redirects are opt-in and re-authorized
#[tokio::test]
async fn http_status_retry_redirect() {
    let (port, stats) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let policy = net_policy(std::slice::from_ref(&base));
    let e = run(
        &op(&format!(
            "r = http get \"{base}/users/500\"\n    decode json\nend\nreturn r"
        )),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!((e.kind, e.code.as_str()), (ErrorKind::Http, "http.status"));
    assert_eq!(e.details.get("status"), Some(&Value::Int(500)));
    assert_eq!(e.http_status(), 502);
    assert_eq!(e.exit_code(), 5);

    let v = run(
        &op(&format!(
            "r = http get \"{base}/users/7\"\n    retry 3 on status [503] backoff exponential base \"10ms\" max \"50ms\" jitter true\n    decode json\nend\nreturn r.body"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v.get("name"), Some(&Value::text("Retry")));
    assert_eq!(stats.flaky.load(Ordering::SeqCst), 3);

    let e = run(
        &op(&format!(
            "r = http get \"{base}/redirect\"\nreturn r.status"
        )),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.details.get("status"), Some(&Value::Int(302)));
    let v = run(
        &op(&format!(
            "r = http get \"{base}/redirect\"\n    redirect follow limit 2\n    decode json\nend\nreturn r.body"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v.get("id"), Some(&Value::Int(42)));
    let e = run(
        &op(&format!(
            "r = http get \"{base}/redirect-away\"\n    redirect follow limit 2\nend\nreturn r.status"
        )),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "permission.denied");

    let e = run(
        &op(&format!(
            "r = http post \"{base}/users\"\n    retry 2 on status [503]\n    body json {{name: \"x\"}}\nend\nreturn r"
        )),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "validation.http_retry_unsafe");
}

// vhco:test transports.exchange_http -- no policy denies, an ungranted port denies, and a private hostname needs a literal IP grant
#[tokio::test]
async fn http_authorization() {
    let (port, stats) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let tmp = tempfile::tempdir().unwrap();
    let rt = rivet::Runtime::builder()
        .source(
            "app.rivet",
            &op(&format!(
                "r = http get \"{base}/users/42\"\nreturn r.status"
            )),
            tmp.path().to_str().unwrap(),
        )
        .build()
        .unwrap();
    let e = rt.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
    let e = run(
        &op(&format!(
            "r = http get \"{base}/users/42\"\nreturn r.status"
        )),
        &net_policy(&["http://127.0.0.1:1".into()]),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    // `localhost` is the loopback literal: a hostname grant does not name 127.0.0.1.
    let e = run(
        &op(&format!("r = http get \"http://localhost:{port}/users/42\"\nreturn r.status")),
        r#"{"version":1,"grants":[{"capability":"allow_network","targets":["http://example.test:80"]}]}"#,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    assert!(stats.requests.lock().unwrap().is_empty());
}

// vhco:test transports.exchange_http -- HTTPS validates against `tls ca_file` (read through the broker first); without it the system trust store rejects the test CA
#[tokio::test]
async fn https_with_ca_file() {
    let (port, ca) = https_server().await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("certs")).unwrap();
    std::fs::write(tmp.path().join("certs/ca.pem"), ca).unwrap();
    let root = tmp.path().to_str().unwrap();
    let base = format!("https://127.0.0.1:{port}");
    let with_ca = op(&format!(
        "r = http get \"{base}/status\"\n    tls ca_file \"./certs/ca.pem\"\n    decode json\nend\nreturn r.body"
    ));
    let granted = format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["{base}"]}},{{"capability":"allow_read","targets":["./certs/**"]}}]}}"#
    );
    let rt = runtime(&with_ca, root, &granted);
    let c = rt.request("t.run", Value::Null, None).await.unwrap();
    assert_eq!(c.result, Value::object([("secure", Value::Bool(true))]));
    // The CA file is an I/O site of its own: without allow_read it fails before connecting.
    let rt = runtime(&with_ca, root, &net_policy(std::slice::from_ref(&base)));
    let e = rt.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
    let rt = runtime(
        &op(&format!("r = http get \"{base}/status\"\nreturn r.status")),
        root,
        &net_policy(&[base]),
    );
    let e = rt.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Tls);
}

// vhco:test transports.exchange_socket -- newline TCP send/receive, EOF mid-frame is protocol.unexpected_eof, early return closes the connection
#[tokio::test]
async fn tcp_newline_and_cleanup() {
    let (port, closed) = tcp_line_server().await;
    let policy = net_policy(&[format!("tcp://127.0.0.1:{port}")]);
    let v = run(
        &op(&format!(
            "with tcp \"127.0.0.1:{port}\" as conn\n    framing newline max_frame 65536\n    conn.send text \"STATUS\"\n    response = conn.receive text timeout \"2s\"\n    return response\nend"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("OK STATUS"));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        closed.load(Ordering::SeqCst),
        1,
        "return must close the socket"
    );

    let e = run(
        &op(&format!(
            "with tcp \"127.0.0.1:{port}\" as conn\n    framing newline\n    conn.send text \"CUT\"\n    return conn.receive text timeout \"2s\"\nend"
        )),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "protocol.unexpected_eof");

    let e = run(
        &op(&format!(
            "with tcp \"127.0.0.1:{port}\" as conn\n    framing newline\n    return conn.receive text timeout \"100ms\"\nend"
        )),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Timeout);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(closed.load(Ordering::SeqCst), 3, "error paths close too");
}

// vhco:test transports.exchange_socket -- length32 big-endian frames round-trip bytes
#[tokio::test]
async fn tcp_length32() {
    let port = tcp_len_server().await;
    let v = run(
        &op(&format!(
            "with tcp \"127.0.0.1:{port}\" as conn\n    framing length32 endian big max_frame 1048576\n    conn.send bytes (base64.decode \"AAEC\")\n    return conn.receive bytes\nend"
        )),
        &net_policy(&[format!("tcp://127.0.0.1:{port}")]),
    )
    .await
    .unwrap();
    assert_eq!(v, Value::Bytes(vec![2, 1, 0]));
}

// vhco:test transports.exchange_socket -- finish_send half-closes while the response stays readable until the peer FIN ends iteration
#[tokio::test]
async fn tcp_finish_send_raw_iteration() {
    let port = tcp_fin_server().await;
    let v = run(
        &op(&format!(
            "out = []\nwith tcp \"127.0.0.1:{port}\" as conn\n    framing raw\n    conn.send text \"hello\"\n    conn.finish_send\n    for chunk in conn\n        out += [chunk]\n    end\nend\nreturn out"
        )),
        &net_policy(&[format!("tcp://127.0.0.1:{port}")]),
    )
    .await
    .unwrap();
    let Value::List(chunks) = v else {
        panic!("expected a list")
    };
    let mut all = Vec::new();
    for c in chunks {
        match c {
            Value::Bytes(b) => all.extend(b),
            other => panic!("raw chunks are bytes, got {other:?}"),
        }
    }
    assert_eq!(String::from_utf8(all).unwrap(), "got 5 bytes");
}

// vhco:test transports.exchange_socket -- a unix socket with newline JSON framing needs allow_unix
#[tokio::test]
async fn unix_json() {
    let tmp = tempfile::tempdir().unwrap();
    let sock = tmp.path().join("render.sock");
    unix_server(&sock).await;
    let path = sock.to_str().unwrap().to_string();
    let src = op(&format!(
        "with unix \"{path}\" as conn\n    framing newline\n    conn.send json {{action: \"render\"}}\n    return conn.receive json timeout \"2s\"\nend"
    ));
    let v = run(
        &src,
        &format!(
            r#"{{"version":1,"grants":[{{"capability":"allow_unix","targets":["{path}"]}}]}}"#
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        v.get("echo").unwrap().get("action"),
        Some(&Value::text("render"))
    );
    let e = run(
        &src,
        &format!(
            r#"{{"version":1,"grants":[{{"capability":"allow_read","targets":["{path}"]}}]}}"#
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "permission.denied");
}

// vhco:test transports.exchange_socket -- WebSocket request/response, a receive loop to a terminal marker, binary echo
#[tokio::test]
async fn websocket_exchange() {
    let port = ws_server().await;
    let policy = net_policy(&[format!("ws://127.0.0.1:{port}")]);
    let v = run(
        &op(&format!(
            "with websocket \"ws://127.0.0.1:{port}/realtime\" as socket\n    timeout \"5s\"\n    socket.send json {{type: \"ping\"}}\n    event = socket.receive json\n    return event\nend"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::object([("type", Value::text("pong"))]));
    let v = run(
        &op(&format!(
            "text = \"\"\nwith websocket \"ws://127.0.0.1:{port}/realtime\" as socket\n    timeout \"5s\"\n    socket.send json {{type: \"generate\"}}\n    while true\n        event = socket.receive json timeout \"2s\"\n        if event.type == \"done\"\n            break\n        end\n        text += event.delta\n    end\nend\nreturn text"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("Hello"));
    let v = run(
        &op(&format!(
            "with websocket \"ws://127.0.0.1:{port}/voice\" as socket\n    socket.send bytes (base64.decode \"AAEC\")\n    return socket.receive bytes\nend"
        )),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::Bytes(vec![0, 1, 2]));
}

fn exec_policy(extra: &str) -> String {
    format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_exec","targets":["/usr/bin/printf","/bin/cat","/usr/bin/false","/bin/sleep","/usr/bin/yes","/bin/echo"]}}{extra}]}}"#
    )
}

// vhco:test transports.run_process -- argv is data (no shell), stdin/decode round-trip, nonzero exit fails kind process unless accepted
#[tokio::test]
async fn process_one_shot() {
    let policy = exec_policy("");
    let v = run(
        &op("result = command \"/usr/bin/printf\"\n    args [\"%s\", \"hello; echo this stays data\"]\n    timeout \"2s\"\n    decode stdout text\nend\nreturn result.stdout"),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::text("hello; echo this stays data"));

    let v = run(
        &op("result = command \"/bin/cat\"\n    stdin json {action: \"summarize\", n: 1}\n    decode stdout json\n    env {LANG: \"C\"}\nend\nreturn result"),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(
        v.get("stdout").unwrap().get("action"),
        Some(&Value::text("summarize"))
    );
    assert_eq!(v.get("exit"), Some(&Value::Int(0)));
    assert!(v.get("duration_ms").is_some());

    let e = run(&op("r = command \"/usr/bin/false\"\nreturn r"), &policy)
        .await
        .unwrap_err();
    assert_eq!(
        (e.kind, e.code.as_str()),
        (ErrorKind::Process, "process.exit")
    );
    let v = run(
        &op("r = command \"/usr/bin/false\"\n    accept exit [0, 1]\nend\nreturn r.exit"),
        &policy,
    )
    .await
    .unwrap();
    assert_eq!(v, Value::Int(1));

    let start = std::time::Instant::now();
    let e = run(
        &op("r = command \"/bin/sleep\"\n    args [\"5\"]\n    timeout \"200ms\"\nend\nreturn r"),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "timeout.process");
    assert!(start.elapsed() < Duration::from_secs(3));
}

// vhco:test transports.run_process -- exec without a grant, bare names and shell strings never spawn
#[tokio::test]
async fn process_refusals() {
    let policy = exec_policy("");
    let e = run(&op("r = command \"/bin/ls\"\nreturn r"), &policy)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
    let e = run(&op("r = command \"printf\"\nreturn r"), &policy)
        .await
        .unwrap_err();
    assert_eq!(e.code, "validation.process_program");
    let e = run(
        &op("r = command \"/bin/sh\"\n    args [\"-c\", \"echo hi\"]\nend\nreturn r"),
        &policy,
    )
    .await
    .unwrap_err();
    assert_eq!(e.code, "unsupported.shell");
}

// vhco:test transports.run_process -- with a policy present the child is sandboxed: granted reads work, other paths are refused by the OS
#[cfg(target_os = "macos")]
#[tokio::test]
async fn process_sandbox_confines_reads() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap().to_string();
    std::fs::create_dir_all(tmp.path().join("data")).unwrap();
    std::fs::create_dir_all(tmp.path().join("secret")).unwrap();
    std::fs::write(tmp.path().join("data/pub.txt"), "public").unwrap();
    std::fs::write(tmp.path().join("secret/s.txt"), "secret").unwrap();
    let policy = exec_policy(r#",{"capability":"allow_read","targets":["./data/**"]}"#);
    let src = "operation t.run\n    param path text required\n    output json\n    r = command \"/bin/cat\"\n        args [path]\n    end\n    return r.stdout\nend\n".to_string();
    let rt = runtime(&src, &root, &policy);
    let ok = rt
        .request(
            "t.run",
            Value::object([("path", Value::text(format!("{root}/data/pub.txt")))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(ok.result, Value::text("public"));
    let e = rt
        .request(
            "t.run",
            Value::object([("path", Value::text(format!("{root}/secret/s.txt")))]),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "process.exit");
    // Glob grants the backend cannot express exactly refuse before spawning.
    let glob = exec_policy(r#",{"capability":"allow_read","targets":["./data/*.txt"]}"#);
    let rt = runtime(&src, &root, &glob);
    let e = rt
        .request("t.run", Value::object([("path", Value::text("x"))]), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "unsupported.sandbox_backend");
    assert_eq!(e.exit_code(), 5);
}

// vhco:test transports.run_process -- a streamed child is iterated by line and killed/reaped when the loop breaks early
#[tokio::test]
async fn process_stream_break_reaps() {
    let marker = format!("rivet-marker-{}", std::process::id());
    let src = op(&format!(
        "n = 0\nwith command \"/usr/bin/yes\" as process\n    args [\"{marker}\"]\n    stream stdout lines\n    for line in process.stdout\n        n += 1\n        if n == 3\n            break\n        end\n    end\nend\nreturn n"
    ));
    let v = run(&src, &exec_policy("")).await.unwrap();
    assert_eq!(v, Value::Int(3));
    tokio::time::sleep(Duration::from_millis(300)).await;
    let ps = std::process::Command::new("/bin/ps")
        .args(["-axo", "command"])
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&ps.stdout).contains(&marker),
        "the child must be reaped at scope exit"
    );
    let v = run(
        &op("out = []\nwith command \"/usr/bin/printf\" as p\n    args [\"a\\nb\\nc\"]\n    stream stdout lines\n    for line in p.stdout\n        out += [line]\n    end\nend\nreturn out"),
        &exec_policy(""),
    )
    .await
    .unwrap();
    assert_eq!(
        v,
        Value::List(vec![Value::text("a"), Value::text("b"), Value::text("c")])
    );
}

// vhco:test transports.exchange_socket -- Stage C forms (TCP TLS, interactive process, FIFO, reconnect, HTTP/3) fail with typed unsupported before any effect
#[tokio::test]
async fn stage_c_refusals() {
    let (port, closed) = tcp_line_server().await;
    let policy = format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["tcp://127.0.0.1:{port}","ws://127.0.0.1:{port}","http://127.0.0.1:{port}"]}},{{"capability":"allow_exec","targets":["/bin/cat"]}},{{"capability":"allow_pipe","targets":["/tmp/rivet-input.fifo"]}}]}}"#
    );
    let cases = [
        (
            format!("with tcp \"127.0.0.1:{port}\" as conn\n    tls true\n    tls ca_file \"./certs/ca.pem\"\n    framing newline\n    return conn.receive json\nend"),
            "unsupported.tcp_tls",
        ),
        (
            "with command \"/bin/cat\" as process\n    interactive true\n    stream stdout jsonl\n    return 1\nend".to_string(),
            "unsupported.interactive",
        ),
        (
            "with pipe \"/tmp/rivet-input.fifo\" mode read as channel\n    framing newline\n    return channel.receive text timeout \"5s\"\nend".to_string(),
            "unsupported.adapter",
        ),
        (
            format!("with websocket \"ws://127.0.0.1:{port}/events\" as socket\n    reconnect 2 backoff exponential max \"2s\"\n    resume none\n    for event in socket\n        return event\n    end\nend\nreturn null"),
            "unsupported.reconnect",
        ),
    ];
    for (body, code) in cases {
        let e = run(&op(&body), &policy).await.unwrap_err();
        assert_eq!(e.code, code, "{body}");
        assert_eq!(e.kind, ErrorKind::Unsupported);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        closed.load(Ordering::SeqCst),
        0,
        "no connection was attempted"
    );
}

// vhco:test transports.exchange_http -- docs/demos/03-http behaves as its README describes against a local fixture (42 → user, 404 → users.not_found, retry on 503, encoded search)
#[tokio::test]
async fn demo_03_http() {
    let (port, _) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let src = std::fs::read_to_string("docs/demos/03-http/app.rivet")
        .unwrap()
        .replace("https://api.example.com", &base);
    let tmp = tempfile::tempdir().unwrap();
    let rt = runtime(&src, tmp.path().to_str().unwrap(), &net_policy(&[base]));
    let c = rt
        .request("users.get", Value::object([("id", Value::Int(42))]), None)
        .await
        .unwrap();
    assert_eq!(
        c.result,
        Value::object([("id", Value::Int(42)), ("name", Value::text("Ada"))])
    );
    let e = rt
        .request("users.get", Value::object([("id", Value::Int(404))]), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "users.not_found");
    assert_eq!(e.details.get("id"), Some(&Value::Int(404)));
    assert_eq!(e.exit_code(), 5);
    let e = rt
        .request("users.get", Value::object([("id", Value::Int(500))]), None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "http.status");
    let c = rt
        .request("users.get", Value::object([("id", Value::Int(7))]), None)
        .await
        .unwrap();
    assert_eq!(c.result.get("name"), Some(&Value::text("Retry")));
    let c = rt
        .request(
            "users.create",
            Value::object([("name", Value::text("Ada"))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(c.result.get("name"), Some(&Value::text("Ada")));
    let c = rt
        .request(
            "users.search",
            Value::object([("query", Value::text("rust & capy"))]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        c.result,
        Value::List(vec![
            Value::object([
                ("key", Value::text("q")),
                ("value", Value::text("rust & capy"))
            ]),
            Value::object([("key", Value::text("limit")), ("value", Value::text("10"))]),
        ])
    );
}

// vhco:test transports.exchange_socket -- docs/demos/04-streaming socket.ping returns the fixture's pong under a WebSocket grant and is denied under the default policy
#[tokio::test]
async fn demo_04_socket_ping() {
    let port = ws_server().await;
    let src = std::fs::read_to_string("docs/demos/04-streaming/app.rivet")
        .unwrap()
        .replace("wss://api.example.com", &format!("ws://127.0.0.1:{port}"));
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_str().unwrap();
    let rt = runtime(&src, root, &net_policy(&[format!("ws://127.0.0.1:{port}")]));
    let c = rt.request("socket.ping", Value::Null, None).await.unwrap();
    assert_eq!(c.result, Value::object([("type", Value::text("pong"))]));
    let default = std::fs::read_to_string("docs/demos/04-streaming/policy.json").unwrap();
    let rt = runtime(&src, root, &default);
    let e = rt
        .request("socket.ping", Value::Null, None)
        .await
        .unwrap_err();
    assert_eq!(e.code, "permission.denied");
}

// vhco:test transports.exchange_http -- `body multipart … end` sends field and file parts (the file read is authorized by allow_read before connecting; without the grant nothing is sent) and `body xml (xml.element …)` sends escaped application/xml
#[tokio::test]
async fn http_multipart_and_xml_bodies() {
    let (port, stats) = http_server().await;
    let base = format!("http://127.0.0.1:{port}");
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("data")).unwrap();
    std::fs::write(tmp.path().join("data/voice.wav"), b"RIFF-bytes").unwrap();
    let root = tmp.path().to_str().unwrap();
    let src = op(&format!(
        "r = http post \"{base}/form\"\n    body multipart\n        field purpose \"transcription\"\n        file audio \"./data/voice.wav\" \"audio/wav\"\n    end\n    decode json\nend\nreturn r.body"
    ));
    let with_read = format!(
        r#"{{"version":1,"grants":[{{"capability":"allow_network","targets":["{base}"]}},{{"capability":"allow_read","targets":["./data/**"]}}]}}"#
    );
    let rt = runtime(&src, root, &with_read);
    let v = rt.request("t.run", Value::Null, None).await.unwrap().result;
    let ct = v.get("ct").and_then(Value::as_str).unwrap().to_string();
    let boundary = ct
        .strip_prefix("multipart/form-data; boundary=")
        .expect("multipart content type");
    let body = v.get("body").and_then(Value::as_str).unwrap();
    assert!(body.starts_with(&format!("--{boundary}\r\n")), "{body}");
    assert!(
        body.contains("Content-Disposition: form-data; name=\"purpose\"\r\n\r\ntranscription\r\n")
    );
    assert!(body.contains(
        "Content-Disposition: form-data; name=\"audio\"; filename=\"voice.wav\"\r\nContent-Type: audio/wav\r\n\r\nRIFF-bytes\r\n"
    ));
    assert!(body.ends_with(&format!("--{boundary}--\r\n")));

    // Without allow_read the part's file read is denied before anything connects.
    let before = stats.requests.lock().unwrap().len();
    let rt = runtime(&src, root, &net_policy(std::slice::from_ref(&base)));
    let e = rt.request("t.run", Value::Null, None).await.unwrap_err();
    assert_eq!(e.code, "permission.denied");
    assert_eq!(stats.requests.lock().unwrap().len(), before);

    let v = run(
        &op(&format!(
            "r = http post \"{base}/form\"\n    body xml (xml.element \"speak\" {{lang: \"en\"}} [\"Hello & welcome \", (xml.element \"b\" {{}} \"<now>\")])\n    decode json\nend\nreturn r.body"
        )),
        &net_policy(std::slice::from_ref(&base)),
    )
    .await
    .unwrap();
    assert_eq!(v.get("ct"), Some(&Value::text("application/xml")));
    assert_eq!(
        v.get("body"),
        Some(&Value::text(
            "<speak lang=\"en\">Hello &amp; welcome <b>&lt;now&gt;</b></speak>"
        ))
    );
}
