use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use noa_common::syncd_protocol::Notification;
use tokio::net::{unix::OwnedWriteHalf, UnixStream};

pub struct SyncdClient {
    workspace_dir: PathBuf,
    lsp_daemons: HashMap<&'static str /* lang id */, OwnedWriteHalf>,
}

impl SyncdClient {
    pub fn new<F>(workspace_dir: &Path, notification_callback: F) -> SyncdClient
    where
        F: FnMut(Notification),
    {
        SyncdClient {
            workspace_dir: workspace_dir.to_owned(),
            lsp_daemons: HashMap::new(),
        }
    }

    pub fn lsp_request<R: Serialize>(&mut self, request: R) {}
}
