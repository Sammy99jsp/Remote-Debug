use futures_util::StreamExt;
use serde_json::json;
use warp::{
    filters::ws::{self, WebSocket},
    http::StatusCode,
    Filter,
};

use crate::{
    meta::{self, MetaOperation},
    traffic::{arriving, departing},
    util::{self, HandlerBuilder},
};

#[derive(Debug, Clone)]
pub struct TLS {
    pub(crate) port: u16,
    pub(crate) certificate: &'static str,
    pub(crate) private_key: &'static str,
}

impl TLS {
    pub fn new(port: u16, certificate_path: &'static str, private_key_path: &'static str) -> Self {
        Self {
            port,
            certificate: certificate_path,
            private_key: private_key_path,
        }
    }
}

///
/// Helper struct to encompass all neccessary
/// info to be picked up by chrome.
///
pub struct DevToolsServer {
    port: u16,
    tls: Option<TLS>,
    handler_builder: Box<dyn Fn() -> HandlerBuilder + Send + Sync>,
    pub(crate) version: meta::BrowserVersion,
    pub(crate) targets: Vec<meta::Target>,
}

impl DevToolsServer {
    pub fn new(
        version: meta::BrowserVersion,
        targets: Vec<meta::Target>,
        port: u16,
        handler_builder: Box<dyn Fn() -> HandlerBuilder + Send + Sync>,
        tls: impl Into<Option<TLS>>,
    ) -> Self {
        Self {
            port,
            version,
            targets,
            handler_builder,
            tls: tls.into(),
        }
    }

    pub async fn handle_client(websocket: WebSocket, handler: util::HandlerBuilder) {
        // Set up channels for this socket.
        // Mainly for my sanity.
        let (raw_tx, raw_rx) = websocket.split();
        let [reply_tx, err_tx, events_tx] = departing(raw_tx);
        let mut rx = arriving(raw_rx, err_tx);

        let handler = handler.build(events_tx);

        while let Some(req) = rx.recv().await {
            let res = handler.handle_incoming(req);
            reply_tx.send(res.await).await.expect("Could not send reply");
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let builder = Box::leak(self.handler_builder);
        let sockets = warp::path!("devtools" / "page" / String)
            .and(warp::ws())
            .map(|page_id: String, ws: ws::Ws| {
                println!("Web socket for page {}", page_id.as_str());
                // And then our closure will be called when it completes...
                ws.on_upgrade(|w| Self::handle_client(w, builder()))
            });

        let version = self.version.clone();
        let targets = self.targets.clone();
            

        let meta = warp::path!("json" / String).map(move |operation: String| {
            TryInto::<MetaOperation>::try_into(operation)
                .map(|op| {
                    warp::reply::with_status(
                        warp::reply::json(&op.exec(&version, &targets)),
                        StatusCode::OK,
                    )
                })
                .unwrap_or_else(|()| {
                    warp::reply::with_status(warp::reply::json(&json!({})), StatusCode::NOT_FOUND)
                })
        });

        

        let meta_route = warp::path!("json").map(move || {
            warp::reply::with_status(
                warp::reply::json(&MetaOperation::Targets.exec(&self.version, &self.targets)),
                StatusCode::OK,
            )
        });

        let routes = sockets.or(meta).or(meta_route);

        println!("DEBUG > HTTP Running on *:{}", self.port);

        let http = warp::serve(routes.clone()).run(([0, 0, 0, 0], self.port));

        if let Some(ref tls) = self.tls {
            println!("DEBUG > HTTPS Running on *:{}", tls.port);

            let https = warp::serve(routes)
                .tls()
                .cert_path(tls.certificate)
                .key_path(tls.private_key)
                .run(([0, 0, 0, 0], tls.port));

            futures::future::join(http, https).await;
        } else {
            http.await;
        }

        let builder = unsafe {
            Box::from_raw_in(
                builder as *const (dyn Fn() -> HandlerBuilder + Send + Sync)
                    as *mut (dyn Fn() -> HandlerBuilder + Send + Sync),
                std::alloc::Global,
            )
        };
        drop(builder);

        Ok(())
    }
}
