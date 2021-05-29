use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum ToServer<R> {
    Request(Request<R>),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ToClient<R, N> {
    Response(Response<R>),
    Notification(Notification<N>),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Request<T> {
    pub id: usize,
    pub body: T,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Response<T> {
    pub id: usize,
    pub body: T,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Notification<T> {
    pub body: T,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum LspRequest {
    OpenFile {
        path: PathBuf,
        text: String,
    },
    UpdateFile {
        path: PathBuf,
        text: String,
        version: usize,
    },
}

#[derive(Serialize, Debug)]
pub enum LspResponse {
    NoContent,
}

#[derive(Serialize, Debug)]
pub enum LspNotification {}

unsafe impl<T: Send> Send for Request<T> {}
unsafe impl<T: Send> Send for Response<T> {}
unsafe impl<T: Send> Send for Notification<T> {}
unsafe impl Send for LspRequest {}
unsafe impl Send for LspResponse {}
unsafe impl Send for LspNotification {}
