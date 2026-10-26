//! OllamaMapPlanner: local LLM map planner with mockable HTTP transport.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use crate::plan::{AiMapPlan, LOCATION_NAMES};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiMaterialOption {
    pub id: String,
    pub name: String,
}

impl AiMaterialOption {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status_code: u16, body: impl Into<String>) -> Self {
        Self {
            status_code,
            body: body.into(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }
}

pub trait HttpTransport: Send + Sync {
    fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String>;
}

pub struct MockHttpTransport<F>
where
    F: Fn(&str, &str, Option<&str>) -> Result<HttpResponse, String> + Send + Sync,
{
    handler: F,
    request_count: AtomicUsize,
}

impl<F> MockHttpTransport<F>
where
    F: Fn(&str, &str, Option<&str>) -> Result<HttpResponse, String> + Send + Sync,
{
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            request_count: AtomicUsize::new(0),
        }
    }

    pub fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

impl<F> HttpTransport for MockHttpTransport<F>
where
    F: Fn(&str, &str, Option<&str>) -> Result<HttpResponse, String> + Send + Sync,
{
    fn send(
        &self,
        method: &str,
        url: &str,
        _headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        let body_str = body.and_then(|b| std::str::from_utf8(b).ok());
        (self.handler)(method, url, body_str)
    }
}

/// Limits applied by the loopback HTTP/1.1 transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpTransportOptions {
    /// Per-address TCP connect timeout.
    pub connect_timeout: Duration,
    /// Deadline for the whole request/response exchange (C# `HttpClient.Timeout` is 10 minutes).
    pub total_timeout: Duration,
    pub max_header_bytes: usize,
    pub max_body_bytes: usize,
}

impl Default for TcpTransportOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            total_timeout: Duration::from_secs(600),
            max_header_bytes: 64 * 1024,
            max_body_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Plain `http://` HTTP/1.1 transport for local Ollama instances (default options).
///
/// Supports `Content-Length`, `Transfer-Encoding: chunked` (extensions and trailers) and
/// close-delimited bodies, with connect/total timeouts, size limits and strict URL/status parsing.
/// `https://` is rejected explicitly; redirects are not followed (3xx is returned as a status).
pub struct TcpHttpTransport;

/// Same as [`TcpHttpTransport`] with caller-chosen [`TcpTransportOptions`].
pub struct ConfiguredTcpHttpTransport {
    pub options: TcpTransportOptions,
}

impl ConfiguredTcpHttpTransport {
    pub fn new(options: TcpTransportOptions) -> Self {
        Self { options }
    }
}

impl HttpTransport for TcpHttpTransport {
    fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        tcp_send(&TcpTransportOptions::default(), method, url, headers, body)
    }
}

impl HttpTransport for ConfiguredTcpHttpTransport {
    fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        tcp_send(&self.options, method, url, headers, body)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedUrl {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str) -> Result<ParsedUrl, String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("http://") {
        &url[7..]
    } else if lower.starts_with("https://") {
        return Err(format!("不支援 https（僅支援本機 http）：{url}"));
    } else {
        return Err(format!("無效的 URL（必須以 http:// 開頭）：{url}"));
    };
    if rest.chars().any(|c| c.is_control() || c == ' ' || c == '\\') {
        return Err(format!("URL 含有非法字元：{url}"));
    }
    let rest = rest.split('#').next().unwrap_or("");
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) if rest.as_bytes()[i] == b'/' => (&rest[..i], rest[i..].to_string()),
        Some(i) => (&rest[..i], format!("/{}", &rest[i..])),
        None => (rest, "/".to_string()),
    };
    if authority.contains('@') {
        return Err(format!("URL 不允許 userinfo：{url}"));
    }
    let (host, port_str) = if let Some(stripped) = authority.strip_prefix('[') {
        let end = stripped.find(']').ok_or_else(|| format!("無效的 IPv6 位址：{url}"))?;
        let after = &stripped[end + 1..];
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':').ok_or_else(|| format!("無效的 URL：{url}"))?),
        };
        (stripped[..end].to_string(), port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (authority.to_string(), None),
        }
    };
    if host.is_empty() || (!authority.starts_with('[') && host.contains(':')) {
        return Err(format!("URL 缺少有效主機：{url}"));
    }
    let port = match port_str {
        None => 80,
        Some(p) => match p.parse::<u16>() {
            Ok(n) if n != 0 && p.bytes().all(|b| b.is_ascii_digit()) => n,
            _ => return Err(format!("無效的連接埠 '{p}'：{url}")),
        },
    };
    Ok(ParsedUrl { host, port, path })
}

fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

fn remaining(deadline: Instant) -> Result<Duration, String> {
    let now = Instant::now();
    if now >= deadline {
        Err("HTTP 要求逾時".to_string())
    } else {
        Ok(deadline - now)
    }
}

fn io_err(what: &str, e: std::io::Error) -> String {
    match e.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => format!("HTTP {what}逾時"),
        _ => format!("HTTP {what}失敗：{e}"),
    }
}

/// Buffered reader enforcing the overall deadline on every socket read.
struct DeadlineReader {
    stream: TcpStream,
    buf: Vec<u8>,
    pos: usize,
    deadline: Instant,
    eof: bool,
}

impl DeadlineReader {
    fn fill(&mut self) -> Result<bool, String> {
        if self.eof {
            return Ok(false);
        }
        if self.pos > 0 && self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        }
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?.max(Duration::from_millis(1))))
            .map_err(|e| io_err("讀取", e))?;
        let mut chunk = [0u8; 8192];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    self.eof = true;
                    return Ok(false);
                }
                Ok(n) => {
                    self.buf.extend_from_slice(&chunk[..n]);
                    return Ok(true);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                // A reset after the peer already sent everything is indistinguishable from
                // truncation; surface it as an error so framing checks decide.
                Err(e) => return Err(io_err("讀取", e)),
            }
        }
    }

    fn available(&self) -> &[u8] {
        &self.buf[self.pos..]
    }

    /// Reads one CRLF/LF terminated line (without terminator), at most `limit` bytes.
    fn read_line(&mut self, limit: usize) -> Result<Option<String>, String> {
        loop {
            if let Some(i) = self.available().iter().position(|&b| b == b'\n') {
                let mut line = &self.available()[..i];
                if line.last() == Some(&b'\r') {
                    line = &line[..line.len() - 1];
                }
                let s = String::from_utf8_lossy(line).into_owned();
                self.pos += i + 1;
                return Ok(Some(s));
            }
            if self.available().len() > limit {
                return Err("HTTP 標頭過大".to_string());
            }
            if !self.fill()? {
                return if self.available().is_empty() {
                    Ok(None)
                } else {
                    Err("HTTP 回應在行結束前中斷".to_string())
                };
            }
        }
    }

    fn read_exact_into(&mut self, out: &mut Vec<u8>, mut n: usize, max: usize) -> Result<(), String> {
        while n > 0 {
            if self.available().is_empty() && !self.fill()? {
                return Err("HTTP 回應本文提早結束".to_string());
            }
            let take = n.min(self.available().len());
            if out.len() + take > max {
                return Err("HTTP 回應本文超過大小上限".to_string());
            }
            out.extend_from_slice(&self.available()[..take]);
            self.pos += take;
            n -= take;
        }
        Ok(())
    }

    fn read_to_close(&mut self, out: &mut Vec<u8>, max: usize) -> Result<(), String> {
        loop {
            if out.len() + self.available().len() > max {
                return Err("HTTP 回應本文超過大小上限".to_string());
            }
            out.extend_from_slice(&self.buf[self.pos..]);
            self.pos = self.buf.len();
            if !self.fill()? {
                return Ok(());
            }
        }
    }
}

fn read_chunked(r: &mut DeadlineReader, out: &mut Vec<u8>, opts: &TcpTransportOptions) -> Result<(), String> {
    loop {
        let line = r
            .read_line(1024)?
            .ok_or_else(|| "HTTP chunked 本文提早結束".to_string())?;
        let size_str = line.split(';').next().unwrap_or("").trim();
        if size_str.is_empty() || !size_str.bytes().all(|b| b.is_ascii_hexdigit()) || size_str.len() > 16 {
            return Err(format!("無效的 chunk 大小：'{size_str}'"));
        }
        let size = usize::from_str_radix(size_str, 16).map_err(|_| "無效的 chunk 大小".to_string())?;
        if size == 0 {
            // Trailers until the blank line.
            loop {
                match r.read_line(opts.max_header_bytes)? {
                    None => return Err("HTTP chunked 結尾提早結束".to_string()),
                    Some(l) if l.is_empty() => return Ok(()),
                    Some(_) => {}
                }
            }
        }
        r.read_exact_into(out, size, opts.max_body_bytes)?;
        let end = r
            .read_line(2)?
            .ok_or_else(|| "HTTP chunked 本文提早結束".to_string())?;
        if !end.is_empty() {
            return Err("chunk 資料後缺少 CRLF".to_string());
        }
    }
}

fn read_response(
    r: &mut DeadlineReader,
    method: &str,
    opts: &TcpTransportOptions,
) -> Result<HttpResponse, String> {
    loop {
        let status_line = r
            .read_line(opts.max_header_bytes)?
            .ok_or_else(|| "伺服器未回應即關閉連線".to_string())?;
        let mut parts = status_line.splitn(3, ' ');
        let version = parts.next().unwrap_or("");
        let code = parts.next().unwrap_or("");
        if !(version == "HTTP/1.1" || version == "HTTP/1.0")
            || code.len() != 3
            || !code.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(format!("無效的 HTTP 狀態列：'{status_line}'"));
        }
        let status: u16 = code.parse().unwrap();
        if !(100..=599).contains(&status) {
            return Err(format!("無效的 HTTP 狀態碼：{status}"));
        }

        let mut content_length: Option<usize> = None;
        let mut chunked = false;
        let mut encoded = false;
        let mut header_bytes = 0usize;
        loop {
            let line = r
                .read_line(opts.max_header_bytes)?
                .ok_or_else(|| "HTTP 標頭提早結束".to_string())?;
            if line.is_empty() {
                break;
            }
            header_bytes += line.len();
            if header_bytes > opts.max_header_bytes {
                return Err("HTTP 標頭過大".to_string());
            }
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| format!("無效的 HTTP 標頭：'{line}'"))?;
            let value = value.trim();
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => {
                    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(format!("無效的 Content-Length：'{value}'"));
                    }
                    let n: usize = value.parse().map_err(|_| "Content-Length 過大".to_string())?;
                    if content_length.is_some_and(|c| c != n) {
                        return Err("Content-Length 重複且不一致".to_string());
                    }
                    content_length = Some(n);
                }
                "transfer-encoding" => {
                    let codings: Vec<String> =
                        value.split(',').map(|c| c.trim().to_ascii_lowercase()).collect();
                    if codings.last().map(String::as_str) == Some("chunked") {
                        chunked = true;
                        if codings.len() > 1 {
                            encoded = true;
                        }
                    } else {
                        encoded = true;
                    }
                }
                "content-encoding" => {
                    if !value.eq_ignore_ascii_case("identity") {
                        encoded = true;
                    }
                }
                _ => {}
            }
        }
        if encoded {
            return Err("不支援的 Content-Encoding／Transfer-Encoding".to_string());
        }

        if (100..200).contains(&status) && status != 101 {
            continue; // interim response such as 100 Continue
        }
        let mut body = Vec::new();
        let no_body = method.eq_ignore_ascii_case("HEAD") || status == 101 || status == 204 || status == 304;
        if !no_body {
            if chunked {
                read_chunked(r, &mut body, opts)?;
            } else if let Some(n) = content_length {
                if n > opts.max_body_bytes {
                    return Err("HTTP 回應本文超過大小上限".to_string());
                }
                r.read_exact_into(&mut body, n, opts.max_body_bytes)?;
            } else {
                r.read_to_close(&mut body, opts.max_body_bytes)?;
            }
        }
        return Ok(HttpResponse::new(status, String::from_utf8_lossy(&body).into_owned()));
    }
}

fn tcp_send(
    opts: &TcpTransportOptions,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Result<HttpResponse, String> {
    if !is_token(method) {
        return Err(format!("無效的 HTTP 方法：'{method}'"));
    }
    let parsed = parse_http_url(url)?;
    let deadline = Instant::now() + opts.total_timeout;

    let host_for_header = if parsed.host.contains(':') {
        format!("[{}]", parsed.host)
    } else {
        parsed.host.clone()
    };
    let host_header = if parsed.port == 80 {
        host_for_header
    } else {
        format!("{}:{}", host_for_header, parsed.port)
    };

    let body_bytes = body.unwrap_or(&[]);
    let mut request = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        method, parsed.path, host_header
    );
    if body.is_some() || matches!(method.to_ascii_uppercase().as_str(), "POST" | "PUT" | "PATCH") {
        request.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    }
    for &(k, v) in headers {
        if !is_token(k) || v.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(format!("無效的 HTTP 標頭：'{k}'"));
        }
        let lk = k.to_ascii_lowercase();
        if matches!(lk.as_str(), "host" | "content-length" | "connection" | "transfer-encoding" | "accept-encoding") {
            continue; // transport owns framing headers
        }
        request.push_str(&format!("{}: {}\r\n", k, v));
    }
    request.push_str("Accept-Encoding: identity\r\n\r\n");

    let addrs: Vec<_> = (parsed.host.as_str(), parsed.port)
        .to_socket_addrs()
        .map_err(|e| format!("無法解析主機 {}：{}", parsed.host, e))?
        .collect();
    let mut last_err = format!("無法解析主機 {}", parsed.host);
    let mut stream = None;
    for addr in addrs {
        let t = remaining(deadline)?.min(opts.connect_timeout);
        match TcpStream::connect_timeout(&addr, t) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last_err = format!("無法連線到 {}：{}", addr, e),
        }
    }
    let mut stream = stream.ok_or(last_err)?;
    let _ = stream.set_nodelay(true);

    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|e| io_err("寫入", e))?;
    stream.write_all(request.as_bytes()).map_err(|e| io_err("寫入", e))?;
    if !body_bytes.is_empty() {
        stream.write_all(body_bytes).map_err(|e| io_err("寫入", e))?;
    }
    stream.flush().map_err(|e| io_err("寫入", e))?;

    let mut reader = DeadlineReader { stream, buf: Vec::new(), pos: 0, deadline, eof: false };
    read_response(&mut reader, method, opts)
}

/// Mirrors the C# `OllamaMapPlanner.InferenceGate`: one inference request at a time per process.
static INFERENCE_GATE: Mutex<()> = Mutex::new(());

pub struct OllamaMapPlanner {
    endpoint: String,
    transport: Arc<dyn HttpTransport>,
}

impl OllamaMapPlanner {
    pub fn new(base_url: Option<&str>, transport: Option<Arc<dyn HttpTransport>>) -> Self {
        let url = base_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("ARM_OLLAMA_URL").ok())
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let endpoint = url.trim_end_matches('/').to_string();

        let transport = transport.unwrap_or_else(|| Arc::new(TcpHttpTransport));

        Self { endpoint, transport }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn list_models(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/tags", self.endpoint);
        let res = self
            .transport
            .send("GET", &url, &[("Accept", "application/json")], None)?;

        if !res.is_success() {
            return Err(format!("Ollama 回應錯誤 {}：{}", res.status_code, res.body));
        }

        let doc: serde_json::Value = serde_json::from_str(&res.body)
            .map_err(|e| format!("Invalid JSON from api/tags: {}", e))?;

        let mut model_names = Vec::new();
        if let Some(models) = doc.get("models").and_then(|m| m.as_array()) {
            for m in models {
                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                    model_names.push(name.to_string());
                }
            }
        }

        model_names.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
        Ok(model_names)
    }

    pub fn generate_plan(
        &self,
        model: &str,
        description: &str,
        materials: &[AiMaterialOption],
        water_level_sample: f32,
    ) -> Result<(AiMapPlan, String), String> {
        if model.trim().is_empty() {
            return Err("請選擇 AI 模型。".to_string());
        }
        if description.trim().is_empty() {
            return Err("請輸入地圖描述。".to_string());
        }

        let material_ids: Vec<&str> = materials.iter().map(|m| m.id.as_str()).collect();

        let schema = AiMapPlan::json_schema(&material_ids);
        let system_prompt = Self::build_system_prompt(materials, water_level_sample);

        let request_body = serde_json::json!({
            "model": model,
            "stream": false,
            "think": false,
            "format": schema,
            "options": {
                "temperature": 0.4,
                "repeat_penalty": 1.15,
                "num_predict": 2048
            },
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": description.trim()
                }
            ]
        });

        let body_bytes = serde_json::to_vec(&request_body).map_err(|e| e.to_string())?;
        let url = format!("{}/api/chat", self.endpoint);

        let _gate = INFERENCE_GATE.lock().unwrap_or_else(|p| p.into_inner());
        let res = self.transport.send(
            "POST",
            &url,
            &[
                ("Content-Type", "application/json"),
                ("Accept", "application/json"),
            ],
            Some(&body_bytes),
        )?;

        if !res.is_success() {
            return Err(format!("Ollama 回應錯誤 {}：{}", res.status_code, res.body));
        }

        let doc: serde_json::Value = serde_json::from_str(&res.body)
            .map_err(|e| format!("Invalid JSON from api/chat: {}", e))?;

        let content = doc
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let plan = AiMapPlan::parse(&content, &material_ids)?;
        Ok((plan, content))
    }

    pub fn build_system_prompt(materials: &[AiMaterialOption], water_level_sample: f32) -> String {
        let locations_str = LOCATION_NAMES.join(", ");
        let mut prompt = String::new();

        prompt.push_str("You design terrain for a 64x64-tile map of the RTS game Against Rome. You are driving the editor's interactive brushes (height, water, material, collision) step by step like a human designer. Reply ONLY with a JSON map plan matching the schema.\n");
        prompt.push_str(&format!(
            "Place every feature with \"location\" (one of: {}). Map north is the top edge, east is the right edge.\n",
            locations_str
        ));
        prompt.push_str("Linear features (river, ridge, blocked line) go from \"location\" to \"toLocation\". Do not output x/y numbers. Every river and ridge MUST have a toLocation different from its location. Never repeat a feature; use 4-16 features in total.\n");
        prompt.push_str("A feature's optional material is also painted over that feature (e.g. sand along a river, rock on a mountain).\n");
        prompt.push_str(&format!(
            "Heights use a 0-255 scale reachable by brush strokes. Ground below {:.0} becomes water; water features are carved via the editor's water brush at the water level ({:.0}). Typical dry land is {:.0}-{:.0}.\n",
            water_level_sample,
            water_level_sample,
            water_level_sample + 15.0,
            water_level_sample + 90.0
        ));
        prompt.push_str("Optional baseHeight flattens the whole map first (use it for a fresh map, e.g. a value a little above the water level). Optional baseMaterial repaints the whole ground first.\n");
        prompt.push_str("Feature types (radius in tiles, 1-32):\n");
        prompt.push_str("- hill / mountain: raise around the location; amount = height gain (hill ~30-60, mountain ~80-140).\n");
        prompt.push_str("- valley: lower around the location by amount.\n");
        prompt.push_str("- plateau: flatten the area to absolute height amount.\n");
        prompt.push_str("- lake: carve a lake below water; amount = water depth in height units below the water level (5-30, default 16).\n");
        prompt.push_str("- ridge: raised line from location to toLocation; radius = half width; amount = height gain.\n");
        prompt.push_str("- river: water channel from location to toLocation; radius = half width (1-3); amount = water depth in height units below the water level (5-30, default 16).\n");
        prompt.push_str("- material: paint ground material inside the circle; material must be one of the ids below.\n");
        prompt.push_str("- blocked / passable: mark impassable or passable ground (line if toLocation given).\n");
        prompt.push_str("Keep settlements playable: leave several large flat dry areas, avoid covering the whole map with water or mountains, and give each region a distinct location.\n");
        prompt.push_str("Available materials (id: name):\n");
        for mat in materials {
            prompt.push_str(&format!("- {}: {}\n", mat.id, mat.name));
        }
        prompt.push_str("Write the summary in the same language as the user's request.\n");

        prompt
    }
}
