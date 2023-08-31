#![feature(allocator_api)]

pub mod jsonrpc;
pub mod util;
pub mod server;
pub mod meta;
pub mod traffic;

pub use server::{DevToolsServer, TLS};
pub use util::{HandlerBuilder, Handler};
pub use meta::{BrowserVersion, Target};

#[cfg(test)]
mod tests {
    use crate::{DevToolsServer, BrowserVersion, Target, TLS, HandlerBuilder, jsonrpc::Response};

    #[tokio::test]
    async fn test_server() -> anyhow::Result<()> {
        let mut builder = HandlerBuilder::default();
        builder.forward(["Runtime."].iter(), |req, _| {
            println!("{req}");
            Response::default()
        });


        let server = DevToolsServer::new(BrowserVersion::
            default(), vec![Target::default()],
            9002,
            builder,
            TLS {
                port: 9003,
                certificate: "keys/cert.pem",
                private_key: "keys/key.pem",
            }
        );

        server.run().await
    }
}