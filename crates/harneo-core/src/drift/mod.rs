//! Drift: keeping registered projects honest. The live rescan watcher is here; the drift check
//! itself lands with its own ticket.

pub mod watch;

#[cfg(test)]
mod watch_tests;
