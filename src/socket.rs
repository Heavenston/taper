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

pub enum SocketEvent<P> {
    Packet(P),
    InvalidPacket,
    IoError(std::io::Error),
}

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
    #[cfg(feature = "async")]
    pub async fn connect_async(addr: impl net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_stream = TcpStream::connect(addr).await?;
        Ok(Self::from_tcp_stream(tcp_stream))
    }
    pub fn connect(addr: impl std::net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_stream = std::net::TcpStream::connect(addr)?;
        #[cfg(feature = "async")]
        tcp_stream.set_nonblocking(true).unwrap();
        #[cfg(feature = "async")]
        let tcp_stream = TcpStream::from_std(tcp_stream)?;
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

    pub fn packet_sender(&self) -> &Sender<P> {
        &self.packet_sender
    }
    pub fn event_receiver(&self) -> &Receiver<SocketEvent<P>> {
        &self.event_receiver
    }
}
