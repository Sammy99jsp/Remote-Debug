use std::{collections::HashMap, sync::Arc, cell::RefCell};

use crate::jsonrpc::{self, Request, Response};
use chrome_devtools_api::Command;
use tokio::sync::{mpsc::{channel, Receiver, Sender}, RwLock};

pub trait Listener<C: Command>:
    Fn(C::Parameters, Sender<Response>) -> Res<C> + Sync + Send + Clone + 'static
{
}

impl<C, F> Listener<C> for F
where
    C: Command,
    F: Fn(C::Parameters, Sender<Response>) -> Res<C> + Sync + Send + Clone + 'static,
{
}

pub type RawListener = dyn Fn(Request, Sender<Response>) -> Response + Sync + Send + 'static;

type Res<C> = Result<<C as Command>::Returns, <C as Command>::Error>;

pub trait IntoRawListener<C: Command>: Listener<C> {
    fn into_listener(self) -> Arc<RawListener> {
        Arc::new(move |req, events_tx| {
            let m = req.method.clone();
            let id = req.id.clone();

            let p: Result<C::Parameters, Response> = serde_json::from_value(req.params)
                .map_err(|_| jsonrpc::invalid_request(id.clone(), req.method));

            let params = match p {
                Ok(i) => i,
                Err(e) => return e,
            };

            let res = self(params, events_tx);

            res.map(|ret| Response {
                id: id.clone(),
                method: Some(m.clone()),
                result: Some(serde_json::to_value(ret).unwrap()),
                ..Default::default()
            })
            .map_err(|e| {
                let mut err = jsonrpc::invalid_request(id, m);
                if let Some(data) = err.error.as_mut().map(|e| &mut e.data) {
                    let _ = data.insert(serde_json::to_value(e).unwrap());
                }
                err
            })
            .either()
        })
    }
}

impl<C, F> IntoRawListener<C> for F
where
    C: Command,
    F: Listener<C>,
{
}

///
/// DevTools-side of communications.
///
pub struct ForwarderIn {
    ///
    /// Methods/domains to listen for.
    ///
    actions: Vec<String>,

    ///
    /// Incoming requests, and additional tx channel (for notifications).
    ///
    inbound: Sender<(Request, Sender<Response>)>,

    ///
    /// Responses out (only one per request!)
    ///
    outbound: RwLock<Receiver<Response>>,
}

impl ForwarderIn {
    ///
    /// A forwarder contains
    ///
    pub fn has(&self, action: &String) -> bool {
        self.actions
            .iter()
            .any(|d| action.to_lowercase().starts_with(&d.to_lowercase()))
    }

    pub async fn send(&self, req: Request, res: &Sender<Response>) -> Response {
        self.inbound.send((req, res.clone())).await.unwrap();
        let a = self.outbound.write().await.recv().await.unwrap();
        a
    }
}

///
/// V8 Inspector-side of communications.
///
pub struct ForwarderOut {
    ///
    /// Incoming requests, along with an auxiliary tx channel for
    /// Notifications.
    ///
    inbound: Receiver<(Request, Sender<Response>)>,

    ///
    /// Outbound response to requests (only one per request!)
    /// Use inbound.1 for notifications.
    ///
    outbound: Sender<Response>,
}

impl ForwarderOut {
    pub fn incoming(&mut self) -> &mut Receiver<(Request, Sender<Response>)> {
        &mut self.inbound
    }

    pub fn outbound(&self) -> &Sender<Response> {
        &self.outbound
    }
}



///
/// Utility struct to generate the required channels.
///
pub struct Forwarder(Vec<String>);

impl Forwarder {
    pub fn new<T: ToString>(actions: impl IntoIterator<Item = T>) -> Self {
        Self(actions.into_iter().map(|el| el.to_string()).collect())
    }

    pub fn split(self) -> (ForwarderIn, ForwarderOut) {
        let (inbound_in, inbound_out) = channel(8);
        let (outbound_in, outbound_out) = channel(8);

        (
            ForwarderIn {
                actions: self.0,
                inbound: inbound_in,
                outbound: RwLock::new(outbound_out),
            },
            ForwarderOut {
                inbound: inbound_out,
                outbound: outbound_in,
            },
        )
    }
}

#[derive(Default)]
pub struct HandlerBuilder {
    forwarders: Vec<ForwarderIn>,
    handlers: HashMap<String, Arc<RawListener>>,
}

impl HandlerBuilder {
    pub fn new(forwarders: Vec<ForwarderIn>, handlers: HashMap<String, Arc<RawListener>>) -> Self {
        Self {
            forwarders,
            handlers,
        }
    }

    pub fn add_listener<C: Command, L: IntoRawListener<C>>(
        &mut self,
        _: C,
        listener: L,
    ) -> &mut Self {
        self.handlers
            .insert(C::id().to_lowercase(), listener.into_listener());

        self
    }

    pub fn forward(&mut self, forwarder_in: ForwarderIn) -> &mut Self
    where
    {
        self.forwarders.push(forwarder_in);
        self
    }

    pub fn build(self, tx: Sender<Response>) -> Handler {
        Handler {
            forwarders: self.forwarders,
            handlers: self.handlers,
            tx,
        }
    }
}

pub struct Handler {
    forwarders: Vec<ForwarderIn>,
    handlers: HashMap<String, Arc<RawListener>>,
    tx: Sender<Response>,
}

impl Handler {
    pub async fn handle_incoming(&self, req: Request) -> jsonrpc::Response {
        let id = req.id.clone();
        let m = req.method.clone();

        // Give forwarders precedence over normal handlers.
        if let Some(forwarder) = self
            .forwarders
            .iter()
            .find(|f| f.has(&m))
        {
            return forwarder.send(req, &self.tx).await;
        }

        self.handlers
            .get(&req.method.to_lowercase())
            .map(|l| l(req, self.tx.clone()))
            .unwrap_or(jsonrpc::invalid_request(id, m))
    }
}

pub trait Either<T>: Sized {
    fn either(self) -> T;
}

impl<T: std::fmt::Debug> Either<T> for Result<T, T> {
    fn either(self) -> T {
        match self {
            Ok(e) => e,
            Err(e) => e,
        }
    }
}
