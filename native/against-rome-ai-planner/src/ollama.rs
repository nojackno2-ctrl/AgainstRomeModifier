//! OllamaMapPlanner: local LLM map planner with mockable HTTP transport.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

/// Simple TCP-based HTTP/1.1 transport for local Ollama instances.
pub struct TcpHttpTransport;

impl HttpTransport for TcpHttpTransport {
    fn send(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        use std::io::{Read, Write};
        use std::net::TcpStream;

        let parsed_url = url.strip_prefix("http://").unwrap_or(url);
        let (host_port, path) = match parsed_url.find('/') {
            Some(idx) => (&parsed_url[..idx], &parsed_url[idx..]),
            None => (parsed_url, "/"),
        };

        let host_port = if !host_port.contains(':') {
            format!("{}:80", host_port)
        } else {
            host_port.to_string()
        };

        let mut stream = TcpStream::connect(&host_port)
            .map_err(|e| format!("Failed to connect to {}: {}", host_port, e))?;

        let body_bytes = body.unwrap_or(&[]);
        let mut request = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            method,
            path,
            host_port,
            body_bytes.len()
        );

        for &(k, v) in headers {
            request.push_str(&format!("{}: {}\r\n", k, v));
        }
        request.push_str("\r\n");

        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("Write failed: {}", e))?;
        if !body_bytes.is_empty() {
            stream
                .write_all(body_bytes)
                .map_err(|e| format!("Write body failed: {}", e))?;
        }

        let mut response_bytes = Vec::new();
        stream
            .read_to_end(&mut response_bytes)
            .map_err(|e| format!("Read failed: {}", e))?;

        let response_str = String::from_utf8_lossy(&response_bytes);
        let (header_part, body_part) = match response_str.find("\r\n\r\n") {
            Some(idx) => (&response_str[..idx], &response_str[idx + 4..]),
            None => ("", response_str.as_ref()),
        };

        let status_code = header_part
            .lines()
            .next()
            .and_then(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    parts[1].parse::<u16>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);

        Ok(HttpResponse::new(status_code, body_part))
    }
}

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
        prompt.push_str("- lake: carve a lake below water; amount = depth below the water level (5-30).\n");
        prompt.push_str("- ridge: raised line from location to toLocation; radius = half width; amount = height gain.\n");
        prompt.push_str("- river: water channel from location to toLocation; radius = half width (1-3); amount = depth below water.\n");
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
