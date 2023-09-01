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
    use crate::{
        util::Forwarder,
        BrowserVersion, DevToolsServer, HandlerBuilder, Target, TLS,
    };

    #[tokio::test]
    async fn test_server() -> anyhow::Result<()> {
        let server = DevToolsServer::new(BrowserVersion::
            default(), vec![Target::default()],
            9002,
            Box::new(|| {
                let mut builder = HandlerBuilder::default();
                let forwarder = Forwarder::new(["Runtime."].iter());
                let (f_in, mut f_out) = forwarder.split();
                builder.forward(f_in);
        
                // Mock-up V8 Thread.
                tokio::spawn(async move {
                    while let Some(req) = f_out.incoming().recv().await {
                        println!("--> [V8] {req:?}");
                        f_out.outbound().send(Default::default()).await.unwrap();
                    }
                });

                builder
            }),
            TLS {
                port: 9003,
                certificate: "keys/cert.pem",
                private_key: "keys/key.pem",
            }
        );

        server.run().await
    }
}
