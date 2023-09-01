#![feature(allocator_api)]

pub mod jsonrpc;
pub mod meta;
pub mod server;
pub mod traffic;
pub mod util;

pub use meta::{BrowserVersion, Target};
pub use server::{DevToolsServer, TLS};
pub use util::{Handler, HandlerBuilder};

#[cfg(test)]
mod tests {
    use chrome_devtools_api::protocol::{
        self,
        dom::{GetDocumentReturns, Node},
    };

    use crate::{util::Forwarder, BrowserVersion, DevToolsServer, HandlerBuilder, Target, TLS};

    #[tokio::test]
    async fn test_server() -> anyhow::Result<()> {
        let server = DevToolsServer::new(
            BrowserVersion::default(),
            vec![Target::default()],
            9002,
            Box::new(|| {
                let mut builder = HandlerBuilder::default();
                let forwarder = Forwarder::new(["Runtime."]);
                let (f_in, mut f_out) = forwarder.split();
                builder.forward(f_in);

                builder.add_listener(protocol::dom::GetDocument, |_, _| {
                    Ok(GetDocumentReturns {
                        root: Node {
                            node_id: 1,
                            parent_id: None,
                            backend_node_id: 1,
                            node_type: 9,
                            node_name: "DOCUMENT".to_string(),
                            local_name: "document".to_string(),
                            node_value: "".to_string(),
                            child_node_count: Some(1),
                            children: vec![Node {
                                node_id: 2,
                                parent_id: Some(1),
                                backend_node_id: 2,
                                node_type: 1,
                                node_name: "AVDANOS".to_string(),
                                local_name: "AvdanOS".to_string(),
                                ..Default::default()
                            }]
                            .into(),
                            ..Default::default()
                        },
                    })
                });

                // Mock-up V8 Thread.
                tokio::spawn(async move {
                    while let Some((req, _)) = f_out.incoming().recv().await {
                        println!("--> [V8] {req}");
                        f_out.outbound().send(Default::default()).await.unwrap();
                    }
                });

                builder
            }),
            TLS {
                port: 9003,
                certificate: "keys/cert.pem",
                private_key: "keys/key.pem",
            },
        );

        server.run().await
    }
}
