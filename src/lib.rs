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
//! ## Examples
//!
//!
//! ### Server example
//! ```no_run
//! use taper::{Server, ServerEvent, SocketEvent};
//!
//! // Try to bind server for listening on localhost and port 1234 using u32 packets
//! let server = Server::<u32, u32>::bind("127.0.0.1:1234").expect("Could not bind the server");
//!
//! // Wait for the connection of a single socket
//! let socket = match server.event_receiver().recv().unwrap() {
//!     ServerEvent::Socket(socket) => socket,
//!     ServerEvent::IoError(e) => panic!(e),
//! };
//!
//! let packet = match socket.event_receiver().recv().unwrap() {
//!     SocketEvent::Packet(packet) => packet,
//!     SocketEvent::InvalidPacket => panic!("Received an invalid packet"),
//!     SocketEvent::IoError(e) => panic!(e),
//! };
//!
//! // Print the first packet received
//! // With the client example below this would print 10
//! println!("Received {} from the remote socket !", packet);
//!
//! // Reply to the socket
//! // 11 with the client example
//! socket.packet_sender().send(packet + 1).unwrap();
//! ```
//!
//! ### Client Example
//!
//! ```no_run
//! use taper::{Socket, SocketEvent};
//!
//! // Connect to localhost server using u32 packets
//! let socket = Socket::<u32, u32>::connect("127.0.0.1:1234").expect("Could not connect to local server");
//!
//! // Send the value '10'
//! socket.packet_sender().send(10).unwrap();
//!
//! // Wait for a response packet
//! let response_packet = match socket.event_receiver().recv().unwrap() {
//!     SocketEvent::Packet(packet) => packet,
//!     SocketEvent::InvalidPacket => panic!("Received an invalid packet"),
//!     SocketEvent::IoError(e) => panic!(e),
//! };
//!
//! // With the server example above this would be '11'
//! println!("Received the value '{}' from the server", response_packet);
//!
//! ```

mod server;
mod socket;

pub use server::*;
pub use socket::*;
