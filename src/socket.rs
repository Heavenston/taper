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
pub enum SocketEvent<T> {
    /// Sent when a full packet has been received through the [`Socket`]
    Packet(T),
    /// Sent when an invalid packet was received through the [`Socket`]
    InvalidPacket,
    /// Sent when an [`IoError`] occurs while receiving packets [`Socket`]s
    ///
    /// [`IoError`]: struct@std::io::Error
    IoError(std::io::Error),
}

/// Represent a connection to a remote host.
/// Used both for client to server and server to client connections.
///
/// The generics `TSentPacket` and `TRecvPacket` correspond respectively to the type of packets sent and received from the socket.
///
/// Example usage to connect to a remote server:
/// ```no_run
/// use taper::{Socket, SocketEvent};
///
/// // Tries to connect to server on localhost with port 1234 with packets of types u32
/// let socket = Socket::<u32, u32>::connect("127.0.0.1:1234").unwrap();
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
pub struct Socket<TSentPacket, TRecvPacket>
where
    TSentPacket: Serialize + Send + 'static,
    TRecvPacket: DeserializeOwned + Send + 'static,
{
    packet_sender: Option<Sender<TSentPacket>>,
    event_receiver: Option<Receiver<SocketEvent<TRecvPacket>>>,
    #[cfg(not(feature = "async"))]
    join_handles: Vec<std::thread::JoinHandle<()>>,
}
impl<TSentPacket, TRecvPacket> Socket<TSentPacket, TRecvPacket>
where
    TSentPacket: Serialize + Send + 'static,
    TRecvPacket: DeserializeOwned + Send + 'static,
{
    /// Opens a connection to a remote host.
    ///
    /// Async version of [`connect`].
    /// Requires the `async` feature.
    /// Must be executed while being in a tokio runtime.
    #[cfg(feature = "async")]
    pub async fn connect_async(addr: impl net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_stream = TcpStream::connect(addr).await?;
        Ok(Self::from_tcp_stream(tcp_stream))
    }
    /// Opens a connection to a remote host.
    /// Not that, if the `async` feature is enabled, this must be executed while being in a tokio runtime.
    ///
    /// See type-level documentation for usage.
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
                    packet_buffer.clear();
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
                    packet_buffer.clear();
                    bincode::serialize_into(&mut packet_buffer, &packet).unwrap();
                    match write
                        .write_all(&(packet_buffer.len() as u16).to_ne_bytes())
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
        let join_handles = {
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
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            event_sender.send(SocketEvent::IoError(e)).ok();
                            break;
                        }
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
                        Err(error) => match *error {
                            bincode::ErrorKind::Io(e)
                                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                            {
                                event_sender.send(SocketEvent::IoError(e)).ok();
                                break;
                            }
                            _ => send_event!(SocketEvent::InvalidPacket),
                        },
                        Ok(p) => {
                            send_event!(SocketEvent::Packet(p))
                        }
                    }
                }
            });
            let send_handle = std::thread::spawn(move || {
                let mut packet_buffer = Vec::with_capacity(20);
                while let Ok(packet) = packet_receiver.recv() {
                    packet_buffer.clear();
                    bincode::serialize_into(&mut packet_buffer, &packet).unwrap();
                    match write.write_all(&(packet_buffer.len() as u16).to_ne_bytes()) {
                        Ok(..) => (),
                        Err(..) => break,
                    }
                    match write.write_all(&packet_buffer) {
                        Ok(..) => (),
                        Err(..) => break,
                    }
                }
            });

            [send_handle].into()
        };

        #[cfg(not(feature = "async"))]
        return Self {
            packet_sender: Some(packet_sender),
            event_receiver: Some(event_receiver),
            join_handles,
        };
        #[cfg(feature = "async")]
        return Self {
            packet_sender: Some(packet_sender),
            event_receiver: Some(event_receiver),
        };
    }

    /// Get the packet sender flume channel.
    /// Just send packets in it and they will be ultimately sent down the tcp stream.
    pub fn packet_sender(&self) -> &Sender<TSentPacket> {
        self.packet_sender.as_ref().unwrap()
    }
    /// Returns a reference to the SocketEvent receiver flume channel.
    /// Use this to receive packets from your clients.
    pub fn event_receiver(&self) -> &Receiver<SocketEvent<TRecvPacket>> {
        &self.event_receiver.as_ref().unwrap()
    }
}

#[cfg(not(feature = "async"))]
impl<TSentPacket, TRecvPacket> Drop for Socket<TSentPacket, TRecvPacket>
where
    TSentPacket: Serialize + Send + 'static,
    TRecvPacket: DeserializeOwned + Send + 'static,
{
    fn drop(&mut self) {
        self.packet_sender = None;
        self.event_receiver = None;
        self.join_handles.drain(..).for_each(|h| h.join().unwrap());
    }
}
