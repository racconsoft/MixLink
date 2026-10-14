//! Records backend commands for tests.
use std::sync::{Arc, Mutex};

use super::{AudioBackend, BackendCommand};

#[derive(Clone, Default)]
pub struct FakeBackend {
    sent: Arc<Mutex<Vec<BackendCommand>>>,
}

impl FakeBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns and clears everything sent so far.
    pub fn take(&self) -> Vec<BackendCommand> {
        std::mem::take(&mut *self.sent.lock().unwrap())
    }
}

impl AudioBackend for FakeBackend {
    fn send(&self, cmd: BackendCommand) {
        self.sent.lock().unwrap().push(cmd);
    }
}
