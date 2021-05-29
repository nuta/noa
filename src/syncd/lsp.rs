use anyhow::Result;

use crate::eventloop::Daemon;
use serde::{Deserialize, Serialize};

pub struct LspDaemon {}

#[derive(Deserialize, Debug)]
pub enum Request {
    Ping,
}

#[derive(Serialize, Debug)]
pub enum Response {
    Pong,
}

impl LspDaemon {
    pub fn new() -> LspDaemon {
        LspDaemon {}
    }
}

impl Daemon for LspDaemon {
    type Request = Request;
    type Response = Response;

    fn process(&mut self, request: Self::Request) -> Result<Self::Response> {
        Ok(match request {
            Request::Ping => Response::Pong,
        })
    }
}
