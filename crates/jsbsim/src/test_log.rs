// SPDX-License-Identifier: LGPL-2.1-or-later
//! What code under test logs, for unit tests that check it.

use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

/// Keeps the level and message of every event logged on its thread.
struct Capture(Arc<Mutex<Vec<(Level, String)>>>);

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl Subscriber for Capture {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0
            .lock()
            .unwrap()
            .push((*event.metadata().level(), message.0));
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// The level and message of what `run` logs on this thread, and its result.
pub(crate) fn logged<T>(run: impl FnOnce() -> T) -> (Vec<(Level, String)>, T) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let result = tracing::subscriber::with_default(Capture(Arc::clone(&lines)), run);
    let lines = lines.lock().unwrap().clone();
    (lines, result)
}

/// The messages of what `run` logs on this thread, and its result.
pub(crate) fn messages<T>(run: impl FnOnce() -> T) -> (Vec<String>, T) {
    let (lines, result) = logged(run);
    (lines.into_iter().map(|(_, m)| m).collect(), result)
}
