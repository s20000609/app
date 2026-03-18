use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct AbortHandle {
    tx: Option<oneshot::Sender<()>>,
}

impl AbortHandle {
    pub fn new(tx: oneshot::Sender<()>) -> Self {
        Self { tx: Some(tx) }
    }

    pub fn abort(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Default)]
struct AbortRegistryState {
    handles: HashMap<String, AbortHandle>,
    aborted: HashSet<String>,
}

#[derive(Clone)]
pub struct AbortRegistry {
    inner: Arc<Mutex<AbortRegistryState>>,
}

impl AbortRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AbortRegistryState::default())),
        }
    }

    pub fn register(&self, request_id: String) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let handle = AbortHandle::new(tx);

        if let Ok(mut state) = self.inner.lock() {
            state.aborted.remove(&request_id);
            state.handles.insert(request_id, handle);
        }

        rx
    }

    pub fn abort(&self, request_id: &str) -> Result<(), String> {
        if let Ok(mut state) = self.inner.lock() {
            state.aborted.insert(request_id.to_string());
            if let Some(mut handle) = state.handles.remove(request_id) {
                handle.abort();
            }
            Ok(())
        } else {
            Err(crate::utils::err_msg(
                module_path!(),
                line!(),
                "Failed to acquire lock on abort registry",
            ))
        }
    }

    pub fn unregister(&self, request_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            state.handles.remove(request_id);
        }
    }

    pub fn take_aborted(&self, request_id: &str) -> bool {
        if let Ok(mut state) = self.inner.lock() {
            state.aborted.remove(request_id)
        } else {
            false
        }
    }

    pub fn abort_all(&self) {
        if let Ok(mut state) = self.inner.lock() {
            let pending: Vec<(String, AbortHandle)> = state.handles.drain().collect();
            for (request_id, mut handle) in pending {
                state.aborted.insert(request_id);
                handle.abort();
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_registered(&self, request_id: &str) -> bool {
        if let Ok(state) = self.inner.lock() {
            state.handles.contains_key(request_id)
        } else {
            false
        }
    }
}

impl Default for AbortRegistry {
    fn default() -> Self {
        Self::new()
    }
}
