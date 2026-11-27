pub(crate) mod docker;
pub(crate) mod http;
pub(crate) mod time;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct HostContext {
    pub(crate) hosts: Vec<String>,
    pub(crate) deadline: Instant,
    pub(crate) http_agent: ureq::Agent,
    pub(crate) http_cache: Arc<Mutex<HashMap<String, (Instant, String)>>>,
    pub(crate) containers: Vec<crate::docker::ContainerInfo>,
}

impl HostContext {
    pub(crate) fn remaining_budget(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub(crate) fn http_timeout(&self) -> Duration {
        Duration::from_secs(3).min(self.remaining_budget())
    }
}
