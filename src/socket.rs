#[cfg(not(feature = "async"))]
use std::io::{Read, Write};
#[cfg(not(feature = "async"))]
use std::net;
#[cfg(feature = "async")]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net,
};

use flume::{Receiver, Sender};
use net::TcpStream;
use serde::{de::DeserializeOwned, Serialize};

/// Event sent by a [`Socket`]
pub enum SocketEvent<P> {
    /// Sent when a full packet has been received through the [`Socket`]
    Packet(P),
    /// Sent when an invalid packet was received through the [`Socket`]
    InvalidPacket,
    /// Sent when an IoError happens while reading packets
    IoError(std::io::Error),
}

/// Represent a connection to a remote host
/// Used both for client to server and server to client connections
/// The generic `P` represent the Packet type, it must implement both [`Serialize`] and [`DeserializeOwned`] (so it can be sent and received across the network)
///
/// Example usage to connect to a remote server:
/// ```no_run
/// use taper::{Socket, SocketEvent};
///
/// // Tries to connect to server on localhost with port 1234
/// // with packets of types [`u32`]
/// let socket = Socket::<u32>::connect("127.0.0.1:1234").unwrap();
///
/// // Send a packet to the server
/// socket.packet_sender().send(56745).unwrap();
///
/// // Receive and log packets/errors
/// loop {
///     match socket.event_receiver().recv().unwrap() {
///         SocketEvent::Packet(packet) => println!("Received a packet from the server: {}", packet),
///         SocketEvent::InvalidPacket => println!("The server sent an invalid packet :("),
///         SocketEvent::IoError(error) => println!("An error occurred {}", error),
///     }
/// }
/// ```
pub struct Socket<P>
where
    P: Serialize + Send + 'static,
{
    packet_sender: Sender<P>,
    event_receiver: Receiver<SocketEvent<P>>,
}
impl<P> Socket<P>
where
    P: Serialize + DeserializeOwned + Send + 'static,
{
    /// Opens a connection to a remote host
    ///
    /// Async version of [`connect`]
    /// Requires the `async` feature
    #[cfg(feature = "async")]
    pub async fn connect_async(addr: impl net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_stream = TcpStream::connect(addr).await?;
        Ok(Self::from_tcp_stream(tcp_stream))
    }
    /// Opens a connection to a remote host
    ///
    /// See type-level documentation for usage
    pub fn connect(addr: impl std::net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_stream = std::net::TcpStream::connect(addr)?;
        #[cfg(feature = "async")]
        let tcp_stream = {
            tcp_stream.set_nonblocking(true).unwrap();
            TcpStream::from_std(tcp_stream)?
        };
        Ok(Self::from_tcp_stream(tcp_stream))
    }

    pub(crate) fn from_tcp_stream(stream: TcpStream) -> Self {
        let (packet_sender, packet_receiver) = flume::unbounded();
        let (event_sender, event_receiver) = flume::unbounded();

        #[cfg(feature = "async")]
        {
            let (mut read, mut write) = stream.into_split();
            // Packet (events) receiving task
            tokio::spawn(async move {
                let mut packet_size_buf = [0; std::mem::size_of::<u16>()];
                let mut packet_buffer = Vec::with_capacity(20);

                loop {
                    macro_rules! send_event {
                        ($event:expr) => {
                            match event_sender.send_async($event).await {
                                Ok(..) => continue,
                                Err(..) => break,
                            }
                        };
                    }

                    match read.read_exact(&mut packet_size_buf).await {
                        Ok(..) => {}
                        Err(error) => {
                            send_event!(SocketEvent::IoError(error))
                        }
                    }
                    let packet_size = u16::from_ne_bytes(packet_size_buf);
                    let mut take_stream = (&mut read).take(packet_size as u64);
                    packet_buffer.reserve(packet_size as usize);
                    match take_stream.read_to_end(&mut packet_buffer).await {
                        Ok(..) => (),
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            send_event!(SocketEvent::InvalidPacket)
                        }
                        Err(e) => match event_sender.send_async(SocketEvent::IoError(e)).await {
                            Ok(..) => continue,
                            Err(..) => break,
                        },
                    };

                    match bincode::deserialize(&packet_buffer) {
                        Err(..) => {
                            send_event!(SocketEvent::InvalidPacket)
                        }
                        Ok(p) => {
                            send_event!(SocketEvent::Packet(p))
                        }
                    }
                }
            });
            // Packet send task
            tokio::spawn(async move {
                let mut packet_buffer = Vec::with_capacity(20);
                while let Ok(packet) = packet_receiver.recv_async().await {
                    bincode::serialize_into(&mut packet_buffer, &packet).unwrap();
                    match write
                        .write_all(&(packet_buffer.len() as u32).to_ne_bytes())
                        .await
                    {
                        Ok(..) => (),
                        Err(..) => break,
                    }
                    match write.write_all(&packet_buffer).await {
                        Ok(..) => (),
                        Err(..) => break,
                    }
                }
            });
        }
        #[cfg(not(feature = "async"))]
        {
            let (mut read, mut write) = (stream.try_clone().unwrap(), stream);
            std::thread::spawn(move || {
                let mut packet_size_buf = [0; std::mem::size_of::<u16>()];
                loop {
                    macro_rules! send_event {
                        ($event:expr) => {
                            match event_sender.send($event) {
                                Ok(..) => continue,
                                Err(..) => break,
                            }
                        };
                    }

                    match read.read_exact(&mut packet_size_buf) {
                        Ok(..) => {}
                        Err(error) => {
                            send_event!(SocketEvent::IoError(error))
                        }
                    }
                    let packet_size = u16::from_ne_bytes(packet_size_buf);
                    let mut take_stream = (&mut read).take(packet_size as u64);
                    match bincode::deserialize_from(&mut take_stream) {
                        Ok(..) if take_stream.limit() > 0 => {
                            send_event!(SocketEvent::InvalidPacket)
                        }
                        Err(..) => {
                            send_event!(SocketEvent::InvalidPacket)
                        }
                        Ok(p) => {
                            send_event!(SocketEvent::Packet(p))
                        }
                    }
                }
            });
            std::thread::spawn(move || {
                let mut packet_buffer = Vec::with_capacity(20);
                while let Ok(packet) = packet_receiver.recv() {
                    bincode::serialize_into(&mut packet_buffer, &packet).unwrap();
                    match write.write_all(&(packet_buffer.len() as u32).to_ne_bytes()) {
                        Ok(..) => (),
                        Err(..) => break,
                    }
                    match write.write_all(&packet_buffer) {
                        Ok(..) => (),
                        Err(..) => break,
                    }
                }
            });
        }

        Self {
            packet_sender,
            event_receiver,
        }
    }

    /// Get the packet sender flume channel
    /// Just send packets in it and they will be ultimately sent down the tcp stream
    pub fn packet_sender(&self) -> &Sender<P> {
        &self.packet_sender
    }
    /// Returns a reference to the SocketEvent receiver flume channel
    /// Use this to receive packets from your clients
    pub fn event_receiver(&self) -> &Receiver<SocketEvent<P>> {
        &self.event_receiver
    }
}
