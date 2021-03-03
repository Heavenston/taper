//! # Taper
//! Simple and easy tcp, packet based network communication
//!
//! See [`Server`] and [`Socket`] documentations for example usage

mod server;
mod socket;

pub use server::*;
pub use socket::*;
