//!
//! Handy type definitions for JSONRPC.
//! 

use chrome_devtools_api::Event;
use colored::{Color, Colorize};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
    pub(crate) data: Option<serde_json::Value>,
}

impl RpcError {
    pub fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        }
    }
}

fn empty_obj() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

///
/// Incoming request.
/// 
#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    #[serde(default = "def_version")]
    jsonrpc: String,
    pub(crate) method: String,
    #[serde(default = "empty_obj")]
    pub(crate) params: serde_json::Value,
    pub(crate) id: Option<serde_json::Value>,
}

impl Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\t{}({})",
            self.id
                .as_ref()
                .map(|i| format!("({i})").bold().blue())
                .unwrap_or_default(),
            self.method.bold().yellow(),
            self.params.to_string().cyan()
        )
    }
}

fn def_version() -> String {
    "2.0".to_string()
}

///
/// Outgoing reponse (and/or notification).
/// 
/// TODO: Change this to an enum {Response, Notification}
/// 
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    #[serde(default = "def_version")]
    pub(crate) jsonrpc: String,
    pub(crate) method: Option<String>,
    pub(crate) result: Option<serde_json::Value>,
    pub(crate) params: Option<serde_json::Value>,
    pub(crate) error: Option<RpcError>,
    pub(crate) id: Option<serde_json::Value>,
}

impl Response {
    pub fn reply(req: &Request, res: impl Into<Option<serde_json::Value>>) -> Self {
        Self {
            method: Some(req.method.clone()),
            result: Some(res.into().unwrap_or(json!({}))),
            id: req.id.clone(),
            ..Default::default()
        }
    }
}

impl Default for Response {
    fn default() -> Self {
        Self {
            jsonrpc: "2.0".to_string().into(),
            method: "".to_string().into(),
            result: Some(json!({})),
            params: None,
            error: None,
            id: Some(0.into()),
        }
    }
}

impl<E: Event> From<E> for Response {
    fn from(value: E) -> Self {
        Self {
            jsonrpc: "2.0".to_string().into(),
            method: E::__id().to_string().into(),
            result: None,
            params: Some(serde_json::to_value(value).expect("Error Serializing Event")),
            error: None,
            id: None,
        }
    }
}

impl Response {
    pub fn from(
        res: Result<serde_json::Value, RpcError>,
        id: impl Into<Option<serde_json::Value>>,
    ) -> Self {
        match res {
            Ok(r) => Self {
                result: Some(r),
                id: id.into(),
                ..Default::default()
            },
            Err(e) => Self {
                result: None,
                error: Some(e),
                id: id.into(),
                ..Default::default()
            },
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn has_id(&self) -> bool {
        self.id.is_some()
    }
}

fn shorten(s: String) -> String {
    if s.len() > 250 {
        format!("{}...", &s[0..=250])
    } else {
        s
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\t{}({}{})",
            self.id
                .as_ref()
                .map(|i| format!("({i})").bold().blue())
                .unwrap_or_default(),
            self.method
                .as_ref()
                .map(|m| m.bold().color(if self.error.is_some() {
                    Color::Red
                } else {
                    Color::Yellow
                }))
                .unwrap_or_default(),
            self.result
                .as_ref()
                .map(|s| shorten(s.to_string()).cyan())
                .unwrap_or_else(|| self
                    .error
                    .as_ref()
                    .map(|e| format!("<ERROR> {}", &e.message).red())
                    .unwrap_or_default()),
            self.params
                .as_ref()
                .map(|s| shorten(s.to_string()).cyan())
                .unwrap_or_default()
        )
    }
}

pub fn parse_error() -> Response {
    Response {
        jsonrpc: "2.0".to_string().into(),
        method: None,
        result: None,
        params: None,
        error: Some(RpcError {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }),
        id: None,
    }
}

pub fn invalid_request(
    id: impl Into<Option<serde_json::Value>>,
    method: impl Into<Option<String>>,
) -> Response {
    Response {
        id: id.into(),
        method: method.into(),
        error: Some(RpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        }),
        ..Default::default()
    }
}

pub fn method_not_found(id: Option<serde_json::Value>) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "error": {
            "code": -32601,
            "message": "Method not found",
        },
        "id": id
    })
}
