use futures_util::StreamExt;
use serde_json::json;
use warp::{filters::ws::{WebSocket, self}, http::StatusCode, Filter};

use crate::{meta::{self, MetaOperation}, traffic::{departing, arriving}, util::{self, HandlerBuilder}};

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
    handler_builder: HandlerBuilder,
    pub(crate) version: meta::BrowserVersion,
    pub(crate) targets: Vec<meta::Target>,
}

impl Clone for DevToolsServer {
    fn clone(&self) -> Self {
        Self {
            port: self.port,
            tls: self.tls.clone(),
            handler_builder: self.handler_builder.clone(),
            version: self.version.clone(),
            targets: self.targets.clone(),
        }
    }
}

impl DevToolsServer {
    pub fn new(
        version: meta::BrowserVersion,
        targets: Vec<meta::Target>,
        port: u16,
        handler_builder: HandlerBuilder,
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
            reply_tx.send(res).await.expect("Could not send reply");
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let builder = Box::leak(Box::new(self.handler_builder.clone()));
        let sockets = warp::path!("devtools" / "page" / String)
            .and(warp::ws())
            .map(|page_id: String, ws: ws::Ws| {
                println!("Web socket for page {}", page_id.as_str());
                // And then our closure will be called when it completes...
                ws.on_upgrade(|w| Self::handle_client(w, builder.clone()))
            });

        let self_cloned = self.clone();
        let self_cloned_2 = self.clone();
        let meta = warp::path!("json" / String).map(move |operation: String| {
            TryInto::<MetaOperation>::try_into(operation)
                .map(|op| {
                    warp::reply::with_status(
                        warp::reply::json(&op.exec(&self_cloned)),
                        StatusCode::OK,
                    )
                })
                .unwrap_or_else(|()| {
                    warp::reply::with_status(
                        warp::reply::json(&json!({})),
                        StatusCode::NOT_FOUND,
                    )
                })
        });

        let meta_route = warp::path!("json").map(move || {
            warp::reply::with_status(
                warp::reply::json(&MetaOperation::Targets.exec(&self_cloned_2)),
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

        let builder = unsafe { Box::from_raw_in(builder as *const HandlerBuilder as *mut HandlerBuilder, std::alloc::Global) };
        drop(builder);

        Ok(())
    }
}
