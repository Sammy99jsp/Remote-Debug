use std::{collections::HashMap, sync::Arc};

use crate::jsonrpc::{self, Request, Response};
use chrome_devtools_api::Command;
use tokio::sync::mpsc::Sender;

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

#[derive(Clone, Default)]
pub struct HandlerBuilder {
    forwarders: Vec<(Vec<String>, Arc<RawListener>)>,
    handlers: HashMap<String, Arc<RawListener>>,
}

impl HandlerBuilder {
    pub fn new(
        forwarders: Vec<(Vec<String>, Arc<RawListener>)>,
        handlers: HashMap<String, Arc<RawListener>>,
    ) -> Self {
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

    pub fn forward<S, F>(&mut self, iter: impl Iterator<Item = S>, listener: F) -> &mut Self
    where
        S: ToString,
        F: Fn(Request, Sender<Response>) -> Response + Sync + Send + 'static,
    {
        let discrims = iter.map(|s| s.to_string()).collect::<Vec<_>>();
        let listener: Arc<RawListener> = Arc::new(listener);
        self.forwarders.push((discrims, listener));
        self
    }


    pub fn build(self, tx: Sender<Response>) -> Handler {
        Handler { forwarders: self.forwarders, handlers: self.handlers, tx }
    }
}

pub struct Handler {
    forwarders: Vec<(Vec<String>, Arc<RawListener>)>,
    handlers: HashMap<String, Arc<RawListener>>,
    tx: Sender<Response>,
}

impl Handler {
    
    pub fn handle_incoming(&self, req: Request) -> jsonrpc::Response {
        let id = req.id.clone();
        let m = req.method.clone();

        // Give forwarders precedence over normal handlers.
        if let Some(func) = self
            .forwarders
            .iter()
            .find(|(d, _)| {
                d.iter()
                    .any(|d| m.to_lowercase().starts_with(&d.to_lowercase()))
            })
            .map(|(_, f)| f)
        {
            return func(req, self.tx.clone());
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
