use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::sync::mpsc::{Receiver, Sender};
use warp::ws::{Message, WebSocket};

use crate::jsonrpc::{self, Response};

#[cfg(feature = "logging")]
pub const LOGGING: bool = true;

#[cfg(not(feature = "logging"))]
pub const LOGGING: bool = false;

pub fn departing(mut out: SplitSink<WebSocket, Message>) -> [Sender<jsonrpc::Response>; 3] {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<jsonrpc::Response>(16);

    tokio::spawn(async move {
        while let Some(res) = rx.recv().await {
            out.send(Message::text(
                &serde_json::to_string(&res).expect("Could not Serialize Response"),
            ))
            .await
            .expect("Could not send off Response");

            // Debug print.
            if LOGGING {
                println!("--> {res}");
            }
        }
    });

    [tx.clone(), tx.clone(), tx]
}

pub fn arriving(
    mut arriving: SplitStream<WebSocket>,
    err_tx: Sender<jsonrpc::Response>,
) -> Receiver<jsonrpc::Request> {
    let (tx, rx) = tokio::sync::mpsc::channel::<jsonrpc::Request>(16);

    tokio::spawn(async move {
        while let Some(msg) = arriving.next().await.and_then(Result::ok) {
            let msg = match msg.to_str() {
                Ok(st) => st,
                Err(_) => {
                    err_tx
                        .send(jsonrpc::parse_error())
                        .await
                        .expect("Could not send Parse Error");
                    continue;
                }
            };

            let req = match serde_json::from_str(msg) {
                Ok(req) => req,
                Err(_) => {
                    err_tx
                        .send(jsonrpc::invalid_request(None, None))
                        .await
                        .expect("Could not send Parse Error");
                    continue;
                }
            };

            // Debug print.
            if LOGGING {
                println!("<-- {req}");
            }

            tx.send(req).await.expect("Couldn't send incoming request");
        }
    });

    rx
}

pub fn dispatch_event(tx: &Sender<Response>, event: impl Into<Response> + Send + 'static) {
    let tx = tx.clone();
    tokio::spawn(async move {
        tx.send(event.into()).await.expect("Could not send event");
    });
}
