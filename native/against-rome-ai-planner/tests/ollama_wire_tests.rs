//! Real loopback HTTP wire tests: an in-process `TcpListener` plays Ollama, so the production
//! `TcpHttpTransport` is exercised end to end (no mock trait).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use against_rome_ai_planner::ollama::{
    AiMaterialOption, ConfiguredTcpHttpTransport, HttpTransport, OllamaMapPlanner, TcpHttpTransport,
    TcpTransportOptions,
};

struct Captured {
    head: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Captured {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "client closed before sending headers");
        data.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let len = head
        .lines()
        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
        .unwrap_or(0);
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < len {
        let n = stream.read(&mut buf).unwrap();
        assert!(n > 0, "client closed mid body");
        body.extend_from_slice(&buf[..n]);
    }
    Captured { head, body }
}

/// Serves one connection: captures the request, then writes each script piece with a short pause
/// (forcing fragmented TCP reads). `hold` keeps the socket open after the script.
fn serve(pieces: Vec<Vec<u8>>, hold: Duration) -> (String, JoinHandle<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let h = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let cap = read_request(&mut s);
        for p in pieces {
            let _ = s.write_all(&p);
            let _ = s.flush();
            thread::sleep(Duration::from_millis(15));
        }
        thread::sleep(hold);
        cap
    });
    (base, h)
}

fn chunked(parts: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut out = vec![b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec()];
    for p in parts {
        out.push(format!("{:X};ext=1\r\n", p.len()).into_bytes());
        out.push(p.to_vec());
        out.push(b"\r\n".to_vec());
    }
    out.push(b"0\r\nX-Trailer: yes\r\n\r\n".to_vec());
    out
}

#[test]
fn chunked_response_with_extensions_trailers_and_split_utf8() {
    let json = r#"{"models":[{"name":"Zeta"},{"name":"alpha-模型"},{"name":"beta"}]}"#.as_bytes();
    // Split inside the 3-byte "模" sequence so each chunk is invalid UTF-8 on its own.
    let cut = json.iter().position(|&b| b == 0xE6).unwrap() + 1;
    let (base, h) = serve(chunked(&[&json[..10], &json[10..cut], &json[cut..]]), Duration::ZERO);
    let planner = OllamaMapPlanner::new(Some(&base), None);
    let models = planner.list_models().unwrap();
    assert_eq!(vec!["alpha-模型", "beta", "Zeta"], models);
    let cap = h.join().unwrap();
    assert!(cap.head.starts_with("GET /api/tags HTTP/1.1\r\n"));
    assert!(cap.head.contains(&base["http://".len()..]), "Host header carries host:port: {}", cap.head);
    assert!(cap.head.contains("Connection: close"));
}

#[test]
fn generate_plan_posts_wire_request_and_decodes_chunked_reply() {
    let plan = r#"{"baseHeight":70,"baseMaterial":"Grass","features":[{"type":"hill","x":32,"y":32,"radius":6,"amount":50}]}"#;
    let reply = serde_json::json!({ "message": { "content": plan } }).to_string();
    let bytes = reply.as_bytes();
    let (base, h) = serve(chunked(&[&bytes[..20], &bytes[20..60], &bytes[60..]]), Duration::ZERO);
    let planner = OllamaMapPlanner::new(Some(&format!("{base}/")), None);
    let (parsed, raw) = planner
        .generate_plan("m1", "  a valley  ", &[AiMaterialOption::new("Grass", "草地")], 60.0)
        .unwrap();
    assert_eq!(plan, raw);
    assert_eq!(Some(70), parsed.base_height);
    let cap = h.join().unwrap();
    assert!(cap.head.starts_with("POST /api/chat HTTP/1.1\r\n"), "{}", cap.head);
    assert!(cap.head.contains(&format!("Content-Length: {}", cap.body.len())));
    assert!(cap.head.to_ascii_lowercase().contains("content-type: application/json"));
    let doc: serde_json::Value = serde_json::from_slice(&cap.body).unwrap();
    assert_eq!("m1", doc["model"]);
    assert_eq!("a valley", doc["messages"][1]["content"]);
    assert_eq!(false, doc["stream"]);
}

#[test]
fn content_length_response_returns_without_waiting_for_socket_close() {
    let body = r#"{"models":[]}"#;
    let resp = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
    let (base, h) = serve(vec![resp.into_bytes()], Duration::from_secs(3));
    let t = Instant::now();
    let r = TcpHttpTransport.send("GET", &format!("{base}/api/tags"), &[], None).unwrap();
    assert!(t.elapsed() < Duration::from_secs(2), "must not block until close");
    assert_eq!((200, body), (r.status_code, r.body.as_str()));
    h.join().unwrap();
}

#[test]
fn close_delimited_body_and_http10_are_read_to_eof() {
    let (base, h) = serve(vec![b"HTTP/1.0 200 OK\r\n\r\nhello ".to_vec(), b"world".to_vec()], Duration::ZERO);
    let r = TcpHttpTransport.send("GET", &format!("{base}/x"), &[], None).unwrap();
    assert_eq!("hello world", r.body);
    h.join().unwrap();
}

#[test]
fn interim_100_continue_is_skipped() {
    let (base, h) = serve(
        vec![b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec()],
        Duration::ZERO,
    );
    let r = TcpHttpTransport.send("POST", &format!("{base}/x"), &[], Some(b"{}")).unwrap();
    assert_eq!((200, "ok"), (r.status_code, r.body.as_str()));
    h.join().unwrap();
}

#[test]
fn error_status_surfaces_code_and_body_through_planner() {
    let (base, h) = serve(
        vec![b"HTTP/1.1 404 Not Found\r\nContent-Length: 21\r\n\r\nmodel 'x' not found!!".to_vec()],
        Duration::ZERO,
    );
    let planner = OllamaMapPlanner::new(Some(&base), None);
    let err = planner.generate_plan("x", "d", &[], 60.0).unwrap_err();
    assert!(err.contains("404") && err.contains("model 'x' not found"), "{err}");
    h.join().unwrap();
}

#[test]
fn truncated_bodies_are_errors_not_silent_partial_data() {
    // Content-Length promises 50 bytes, peer closes after 5.
    let (base, h) = serve(vec![b"HTTP/1.1 200 OK\r\nContent-Length: 50\r\n\r\nshort".to_vec()], Duration::ZERO);
    let e = TcpHttpTransport.send("GET", &format!("{base}/x"), &[], None).unwrap_err();
    assert!(e.contains("提早結束"), "{e}");
    h.join().unwrap();

    // Chunked stream closed before the terminating 0-chunk.
    let (base, h) = serve(
        vec![b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n".to_vec()],
        Duration::ZERO,
    );
    assert!(TcpHttpTransport.send("GET", &format!("{base}/x"), &[], None).is_err());
    h.join().unwrap();
}

#[test]
fn malformed_responses_are_rejected() {
    for bad in [
        &b"garbage\r\n\r\n"[..],
        b"HTTP/1.1 20 OK\r\n\r\n",
        b"HTTP/1.1 abc OK\r\n\r\n",
        b"HTTP/1.1 700 X\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: nope\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab",
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nZZ\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n",
        b"",
    ] {
        let (base, h) = serve(vec![bad.to_vec()], Duration::ZERO);
        let r = TcpHttpTransport.send("GET", &format!("{base}/x"), &[], None);
        assert!(r.is_err(), "should reject {:?}, got {:?}", String::from_utf8_lossy(bad), r);
        h.join().unwrap();
    }
}

#[test]
fn silent_server_hits_total_timeout() {
    let (base, h) = serve(vec![], Duration::from_secs(2));
    let t = ConfiguredTcpHttpTransport::new(TcpTransportOptions {
        total_timeout: Duration::from_millis(300),
        ..TcpTransportOptions::default()
    });
    let start = Instant::now();
    let e = t.send("GET", &format!("{base}/x"), &[], None).unwrap_err();
    assert!(e.contains("逾時"), "{e}");
    assert!(start.elapsed() < Duration::from_millis(1800));
    h.join().unwrap();
}

#[test]
fn slow_drip_cannot_extend_past_total_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let h = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        read_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n");
        for _ in 0..40 {
            if s.write_all(b"x").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    });
    let t = ConfiguredTcpHttpTransport::new(TcpTransportOptions {
        total_timeout: Duration::from_millis(400),
        ..TcpTransportOptions::default()
    });
    let start = Instant::now();
    assert!(t.send("GET", &format!("{base}/x"), &[], None).is_err());
    assert!(start.elapsed() < Duration::from_millis(1500));
    h.join().unwrap();
}

#[test]
fn body_size_limit_is_enforced() {
    let (base, h) = serve(vec![b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n01234567890123456789".to_vec()], Duration::ZERO);
    let t = ConfiguredTcpHttpTransport::new(TcpTransportOptions { max_body_bytes: 10, ..TcpTransportOptions::default() });
    assert!(t.send("GET", &format!("{base}/x"), &[], None).unwrap_err().contains("上限"));
    h.join().unwrap();
}

#[test]
fn connection_refused_reports_error_quickly() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let start = Instant::now();
    let e = TcpHttpTransport.send("GET", &format!("http://127.0.0.1:{port}/"), &[], None).unwrap_err();
    assert!(e.contains("連線"), "{e}");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn strict_url_and_header_validation_never_touches_the_network() {
    for url in [
        "https://127.0.0.1:11434/api/tags",
        "127.0.0.1:11434/api/tags",
        "ftp://127.0.0.1/",
        "http://",
        "http://:11434/",
        "http://127.0.0.1:0/",
        "http://127.0.0.1:99999/",
        "http://127.0.0.1:abc/",
        "http://127.0.0.1:/",
        "http://user:pw@127.0.0.1/",
        "http://127.0.0.1/a b",
        "http://[::1/",
    ] {
        assert!(TcpHttpTransport.send("GET", url, &[], None).is_err(), "{url}");
    }
    assert!(TcpHttpTransport.send("GE T", "http://127.0.0.1:1/", &[], None).is_err());
    assert!(TcpHttpTransport
        .send("GET", "http://127.0.0.1:1/", &[("X-A", "v\r\nInjected: 1")], None)
        .unwrap_err()
        .contains("標頭"));
    assert!(TcpHttpTransport
        .send("GET", "http://127.0.0.1:1/", &[("Bad Name", "v")], None)
        .unwrap_err()
        .contains("標頭"));
}

#[test]
fn query_string_is_kept_fragment_dropped_and_framing_headers_not_duplicated() {
    let (base, h) = serve(vec![b"HTTP/1.1 204 No Content\r\n\r\n".to_vec()], Duration::ZERO);
    let r = TcpHttpTransport
        .send(
            "POST",
            &format!("{base}?a=1#frag"),
            &[("Host", "evil"), ("Content-Length", "999"), ("X-Custom", "ok")],
            Some(b"abc"),
        )
        .unwrap();
    assert_eq!((204, ""), (r.status_code, r.body.as_str()));
    let cap = h.join().unwrap();
    assert!(cap.head.starts_with("POST /?a=1 HTTP/1.1\r\n"), "{}", cap.head);
    assert_eq!(1, cap.head.to_ascii_lowercase().matches("content-length:").count());
    assert!(!cap.head.contains("evil"));
    assert!(cap.head.contains("X-Custom: ok"));
    assert_eq!(b"abc", &cap.body[..]);
}

#[test]
fn concurrent_inferences_are_serialized_like_csharp_gate() {
    // Two planners hit two servers; the second server must not see its request until the first reply finished.
    let (tx, rx) = mpsc::channel::<(bool, Instant)>();
    let mk = |tx: mpsc::Sender<(bool, Instant)>, delay_ms: u64| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let h = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            read_request(&mut s);
            let _ = tx.send((true, Instant::now()));
            thread::sleep(Duration::from_millis(delay_ms));
            let body = r#"{"message":{"content":"{}"}}"#;
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes());
            let _ = tx.send((false, Instant::now()));
        });
        (base, h)
    };
    let (b1, h1) = mk(tx.clone(), 300);
    let (b2, h2) = mk(tx, 300);
    let t1 = thread::spawn(move || {
        let _ = OllamaMapPlanner::new(Some(&b1), Some(Arc::new(TcpHttpTransport))).generate_plan("m", "d", &[], 60.0);
    });
    let t2 = thread::spawn(move || {
        let _ = OllamaMapPlanner::new(Some(&b2), None).generate_plan("m", "d", &[], 60.0);
    });
    t1.join().unwrap();
    t2.join().unwrap();
    h1.join().unwrap();
    h2.join().unwrap();
    let mut ev: Vec<(bool, Instant)> = rx.try_iter().collect();
    assert_eq!(4, ev.len());
    ev.sort_by_key(|e| e.1);
    let tags: Vec<bool> = ev.iter().map(|e| e.0).collect();
    assert_eq!(vec![true, false, true, false], tags, "second request started before first finished");
}
