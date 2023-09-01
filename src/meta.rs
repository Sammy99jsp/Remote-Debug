//!
//! The HTTP endpoints associated with our remote debug
//! server.
//!
//! [See Chrome DevTools documentation](https://chromedevtools.github.io/devtools-protocol/#endpoints)
//!
//!

use crate::server::DevToolsServer;

///
/// Targets listsed in under this DevTools Server
///
#[derive(Debug, serde::Serialize, Clone)]
pub struct Target {
    pub(crate) description: String,

    #[serde(rename = "devtoolsFrontendUrl")]
    pub(crate) devtools_frontend_url: Option<String>,
    pub(crate) id: String,
    pub(crate) title: String,

    #[serde(rename = "type")]
    pub(crate) target_type: String,
    pub(crate) url: String,

    #[serde(rename = "webSocketDebuggerUrl")]
    pub(crate) web_socket_debugger_url: String,

    #[serde(rename = "faviconUrl")]
    pub(crate) favicon_url: Option<String>,
}

impl Default for Target {
    fn default() -> Self {
        Target {
            id: "TEST-1".to_string(),
            title: "Remote Debug Test".to_string(),
            description: "A test of the devtools remote debug protocol, implemented in Rust! 🦀"
                .to_string(),
            devtools_frontend_url:
                "/devtools/inspector.html?ws=localhost:9002/devtools/page/TEST-1"
                    .to_string()
                    .into(),
            target_type: "other".to_string(),
            favicon_url: "https://www.google.com/favicon.ico".to_string().into(),
            url: "test://remote-debug".to_string(),
            web_socket_debugger_url: "ws://localhost:9002/devtools/page/TEST-1".to_string(),
        }
    }
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct BrowserVersion {
    ///
    /// Format: `${NAME}/${VERSION}`
    ///
    #[serde(rename = "Browser")]
    pub(crate) browser: String,
    ///
    /// We are supporting 1.3
    ///
    #[serde(rename = "Protocol-Version")]
    pub(crate) protocol_version: String,

    #[serde(rename = "User-Agent")]
    pub(crate) user_agent: String,

    #[serde(rename = "V8-Version")]
    pub(crate) v8_version: Option<String>,

    #[serde(rename = "WebKit-Version")]
    pub(crate) webkit_version: Option<String>,

    #[serde(rename = "webSocketDebuggerUrl")]
    pub(crate) web_socket_debugger_url: Option<String>,
}

impl Default for BrowserVersion {
    fn default() -> Self {
        BrowserVersion {
            browser: concat!(
                "Remote-Debug-Test/Remote-Debug-Test ",
                env!("CARGO_PKG_VERSION")
            )
            .to_string(),
            protocol_version: "1.3".to_string(),
            user_agent: "Remote Debug".to_string(),
            v8_version: None,
            webkit_version: None,
            web_socket_debugger_url: None,
        }
    }
}

pub enum MetaOperation {
    Targets,
    Version,
}

impl MetaOperation {
    pub fn exec(&self, version: &BrowserVersion, targets: &[Target]) -> serde_json::Value {
        match self {
            MetaOperation::Targets => serde_json::to_value(targets)
                .expect("Server.targets failed to be serialized!"),
            MetaOperation::Version => serde_json::to_value(version)
                .expect("Server.version failed to be serialized!"),
        }
    }
}

impl TryFrom<String> for MetaOperation {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "version" => Ok(Self::Version),
            "list" => Ok(Self::Targets),
            "" => Ok(Self::Targets),
            _ => Err(()),
        }
    }
}
