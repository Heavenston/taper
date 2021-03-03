//! # Taper
//! Simple and easy tcp, packet based network communication
//!
//! ## Async
//!
//! Taper supports async usage with the `async` feature.
//! When enabled, taper will use Tokio to execute concurrently all packet listener.
//! If disabled, OS Threads are used instead
//!
//! While this may be overkill for Clients, this may be a great performance improvement for Servers.
//!
//! ## Usage
//!
//! See [`Server`] and [`Socket`] documentations for example usage

mod server;
mod socket;

pub use server::*;
pub use socket::*;
