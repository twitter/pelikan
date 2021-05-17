// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Shared structs and helper functions.

use boring::ssl::{SslAcceptor, SslContext, SslFiletype, SslMethod};
use config::TlsConfig;
use mio::Token;
use mio::Waker;
use std::sync::Arc;

#[derive(Clone)]
pub enum Signal {
    Shutdown,
}

pub struct Queue<T> {
    send: crossbeam_channel::Sender<T>,
    recv: crossbeam_channel::Receiver<T>,
}

#[derive(Clone)]
pub struct Sender<T> {
    send: crossbeam_channel::Sender<T>,
}

impl<T> Sender<T> {
    pub fn send(&self, msg: T) -> Result<(), crossbeam_channel::SendError<T>> {
        self.send.send(msg)
    }

    pub fn try_send(&self, msg: T) -> Result<(), crossbeam_channel::TrySendError<T>> {
        self.send.try_send(msg)
    }
}

impl<T> Queue<T> {
    pub fn new(size: usize) -> Self {
        let (send, recv) = crossbeam_channel::bounded(size);
        Self { send, recv }
    }

    pub fn try_recv(&self) -> Result<T, crossbeam_channel::TryRecvError> {
        self.recv.try_recv()
    }

    pub fn sender(&self) -> Sender<T> {
        Sender {
            send: self.send.clone(),
        }
    }
}

use rtrb::*;

pub use rtrb::PopError;
pub use rtrb::PushError;

pub struct Message<T> {
    pub item: T,
    pub token: Token,
}

pub struct BiDiQueue<T, U> {
    pub(crate) send: Producer<Message<T>>,
    pub(crate) recv: Consumer<Message<U>>,
    pub(crate) waker: Arc<Waker>,
}

impl<T, U> BiDiQueue<T, U> {
    pub fn try_send(&mut self, msg: Message<T>) -> Result<(), PushError<Message<T>>> {
        self.send.push(msg)
    }

    pub fn try_recv(&mut self) -> Result<Message<U>, PopError> {
        self.recv.pop()
    }

    // Notify the receiver that messages are on the queue
    pub fn wake(&self) -> Result<(), std::io::Error> {
        self.waker.wake()
    }

    pub fn pending(&self) -> usize {
        self.recv.slots()
    }
}

pub fn ssl_context(config: &TlsConfig) -> Result<Option<SslContext>, std::io::Error> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;

    if let Some(f) = config.certificate_chain() {
        builder.set_ca_file(f)?;
    } else {
        return Ok(None);
    }

    if let Some(f) = config.certificate() {
        builder.set_certificate_file(f, SslFiletype::PEM)?;
    } else {
        return Ok(None);
    }

    if let Some(f) = config.private_key() {
        builder.set_private_key_file(f, SslFiletype::PEM)?;
    } else {
        return Ok(None);
    }

    Ok(Some(builder.build().into_context()))
}
