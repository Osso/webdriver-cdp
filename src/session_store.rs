use crate::session::Session;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct SessionStore {
    pub sessions: Arc<DashMap<String, Session>>,
    pub chrome_port: u16,
    pub external_chrome_url: Arc<RwLock<Option<String>>>,
}

impl SessionStore {
    pub fn new(chrome_port: u16) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            chrome_port,
            external_chrome_url: Arc::new(RwLock::new(None)),
        }
    }
}
