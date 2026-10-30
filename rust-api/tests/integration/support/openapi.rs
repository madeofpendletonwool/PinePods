//! Turn the server's own OpenAPI document into concrete, well-formed requests.
//!
//! Auth checks live inside each handler (there is no auth middleware), so the only way to
//! prove an endpoint rejects strangers is to actually reach the handler — which means the
//! request must get past axum's extractors (path, query, JSON body). The spec describes all
//! of that, so we synthesise a minimal valid request from it for every operation.

use serde_json::{json, Map, Value};

pub struct Operation {
    pub method: axum::http::Method,
    /// Path template as in the spec, e.g. `/api/data/user_details_id/{id}`.
    pub template: String,
    pub secured: bool,
    pub op: Value,
}

impl Operation {
    pub fn label(&self) -> String {
        format!("{} {}", self.method, self.template)
    }
}

pub struct Spec {
    pub doc: Value,
}

/// Values used for parameters/fields by name, so ids point at *something* plausible.
pub type Overrides<'a> = &'a dyn Fn(&str) -> Option<Value>;

impl Spec {
    pub fn load() -> Spec {
        let (_, api) = pinepods_api::openapi_router().split_for_parts();
        Spec { doc: serde_json::to_value(&api).expect("serialize spec") }
    }

    pub fn operations(&self) -> Vec<Operation> {
        let mut out = Vec::new();
        for (path, item) in self.doc["paths"].as_object().unwrap() {
            for (m, op) in item.as_object().unwrap() {
                let method = match m.as_str() {
                    "get" => axum::http::Method::GET,
                    "post" => axum::http::Method::POST,
                    "put" => axum::http::Method::PUT,
                    "delete" => axum::http::Method::DELETE,
                    "patch" => axum::http::Method::PATCH,
                    _ => continue,
                };
                let secured = op
                    .get("security")
                    .and_then(Value::as_array)
                    .is_some_and(|s| !s.is_empty());
                out.push(Operation { method, template: path.clone(), secured, op: op.clone() });
            }
        }
        out.sort_by_key(|o| o.label());
        out
    }

    fn resolve<'a>(&'a self, schema: &'a Value) -> &'a Value {
        let mut s = schema;
        for _ in 0..16 {
            match s.get("$ref").and_then(Value::as_str) {
                Some(r) => {
                    let name = r.trim_start_matches("#/components/schemas/");
                    s = &self.doc["components"]["schemas"][name];
                }
                None => break,
            }
        }
        s
    }

    /// A minimal instance of `schema` (required fields only, first variant of unions).
    pub fn example(&self, schema: &Value, name: &str, ov: Overrides, depth: usize) -> Value {
        if let Some(v) = ov(name) {
            return v;
        }
        let s = self.resolve(schema);
        if depth > 12 {
            return Value::Null;
        }
        if let Some(e) = s.get("example") {
            return e.clone();
        }
        if let Some(e) = s.get("examples").and_then(Value::as_array).and_then(|a| a.first()) {
            return e.clone();
        }
        if let Some(e) = s.get("enum").and_then(Value::as_array).and_then(|a| a.first()) {
            return e.clone();
        }
        for key in ["oneOf", "anyOf", "allOf"] {
            if let Some(variants) = s.get(key).and_then(Value::as_array) {
                // Prefer a non-null variant.
                let pick = variants
                    .iter()
                    .find(|v| self.resolve(v).get("type") != Some(&json!("null")))
                    .or(variants.first());
                if key == "allOf" {
                    let mut merged = Map::new();
                    for v in variants {
                        if let Value::Object(o) = self.example(v, name, ov, depth + 1) {
                            merged.extend(o);
                        }
                    }
                    return Value::Object(merged);
                }
                if let Some(p) = pick {
                    return self.example(p, name, ov, depth + 1);
                }
            }
        }
        let ty = match s.get("type") {
            Some(Value::String(t)) => t.clone(),
            Some(Value::Array(ts)) => ts
                .iter()
                .filter_map(Value::as_str)
                .find(|t| *t != "null")
                .unwrap_or("null")
                .to_string(),
            _ if s.get("properties").is_some() => "object".into(),
            _ => "any".into(),
        };
        match ty.as_str() {
            "object" => {
                let mut obj = Map::new();
                let required: Vec<&str> = s
                    .get("required")
                    .and_then(Value::as_array)
                    .map(|r| r.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if let Some(props) = s.get("properties").and_then(Value::as_object) {
                    for (k, v) in props {
                        if required.contains(&k.as_str()) || ov(k).is_some() {
                            obj.insert(k.clone(), self.example(v, k, ov, depth + 1));
                        }
                    }
                }
                Value::Object(obj)
            }
            "array" => {
                let min = s.get("minItems").and_then(Value::as_u64).unwrap_or(0);
                if min > 0 {
                    let item = self.example(&s["items"], name, ov, depth + 1);
                    Value::Array(vec![item; min as usize])
                } else {
                    json!([])
                }
            }
            "integer" => json!(1),
            "number" => json!(1.0),
            "boolean" => json!(false),
            "string" => match s.get("format").and_then(Value::as_str) {
                Some("date-time") => json!("2025-01-01T00:00:00Z"),
                Some("date") => json!("2025-01-01"),
                Some("uuid") => json!("00000000-0000-0000-0000-000000000001"),
                _ => json!("x"),
            },
            "null" => Value::Null,
            _ => json!({}),
        }
    }

    fn param_value(&self, p: &Value, ov: Overrides) -> String {
        let name = p["name"].as_str().unwrap();
        match self.example(&p["schema"], name, ov, 0) {
            Value::String(s) => s,
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }

    /// Concrete URI for `op` (path params substituted, required query params appended).
    pub fn uri(&self, op: &Operation, ov: Overrides) -> String {
        let mut path = op.template.clone();
        let mut query = Vec::new();
        for p in op.op.get("parameters").and_then(Value::as_array).into_iter().flatten() {
            let name = p["name"].as_str().unwrap();
            let value = self.param_value(p, ov);
            match p["in"].as_str() {
                Some("path") => path = path.replace(&format!("{{{name}}}"), &urlencoding::encode(&value)),
                Some("query") if p.get("required") == Some(&json!(true)) || ov(name).is_some() => {
                    query.push(format!("{}={}", name, urlencoding::encode(&value)))
                }
                _ => {}
            }
        }
        if query.is_empty() {
            path
        } else {
            format!("{path}?{}", query.join("&"))
        }
    }

    /// Request body for `op`: (bytes, content-type), or None if the operation takes none.
    pub fn body(&self, op: &Operation, ov: Overrides) -> Option<(Vec<u8>, &'static str)> {
        let content = op.op.get("requestBody")?.get("content")?.as_object()?;
        if let Some(c) = content.get("application/json") {
            let v = self.example(&c["schema"], "", ov, 0);
            return Some((serde_json::to_vec(&v).unwrap(), "application/json"));
        }
        if content.contains_key("multipart/form-data") {
            // An empty, well-formed multipart body: enough to get past the extractor.
            return Some((
                b"--pinepodsboundary--\r\n".to_vec(),
                "multipart/form-data; boundary=pinepodsboundary",
            ));
        }
        if content.contains_key("text/plain") {
            return Some((b"x".to_vec(), "text/plain"));
        }
        None
    }
}
