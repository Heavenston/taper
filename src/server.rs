#[cfg(not(feature = "async"))]
use std::net;
#[cfg(feature = "async")]
use tokio::net;

use crate::Socket;
use flume::Receiver;
use net::TcpListener;
use serde::{de::DeserializeOwned, Serialize};

/// Event sent by a [`Server`]
pub enum ServerEvent<P>
where
    P: Serialize + Send + 'static,
{
    /// Sent when a new [`Socket`] connects to the server
    Socket(Socket<P>),
    /// Sent when an [`IoError`] occurs while accepting [`Socket`]s
    ///
    /// [`IoError`]: struct@std::io::Error
    IoError(std::io::Error),
}

/// Represent a server capable of accepting remote connections.
/// When bound to an address, it will start accepting new [`Socket`]s and sending them up the [`event_receiver`].
///
/// The `P` generic parameter correspond to the type of the Packets that connecting Sockets will use to communicate,
/// see [`Socket`] documentation for details.
///
/// [`event_receiver`]: method@crate::Server::event_receiver
///
/// Example usage
/// ```no_run
/// use taper::Server;
///
/// // Try to bind server for listening on localhost and port 1234
/// // Using u32 packets
/// let server = Server::<u32>::bind("127.0.0.1:1234").unwrap();
///
/// // Wait for the connection of a single socket
/// let socket = server.event_receiver().recv().unwrap();
/// ```
/// After that, use sockets however you want!
/// See [`Socket`] documentation for more details.
pub struct Server<P>
where
    P: Serialize + Send + 'static,
{
    event_receiver: Receiver<ServerEvent<P>>,
}

impl<P> Server<P>
where
    P: Serialize + DeserializeOwned + Send + 'static,
{
    /// Creates a server bound to the provided addr.
    ///
    /// Async version [`bind`].
    /// Only available with the `async` feature.
    /// Must be executed while being in a tokio runtime.
    #[cfg(feature = "async")]
    pub async fn bind_async(addr: impl net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_listener = TcpListener::bind(addr).await?;
        Ok(Self::from_tcp_listener(tcp_listener))
    }
    /// Creates a server bound to the provided addr.
    /// Not that, if the `async` feature is enabled, this must be executed while being in a tokio runtime.
    ///
    /// See type-level documentation for usage.
    pub fn bind(addr: impl std::net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_listener = std::net::TcpListener::bind(addr)?;
        #[cfg(feature = "async")]
        let tcp_listener = {
            tcp_listener.set_nonblocking(true).unwrap();
            TcpListener::from_std(tcp_listener)?
        };
        Ok(Self::from_tcp_listener(tcp_listener))
    }

    pub(crate) fn from_tcp_listener(listener: TcpListener) -> Self {
        let (event_sender, event_receiver) = flume::unbounded();

        #[cfg(feature = "async")]
        {
            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let socket = Socket::<P>::from_tcp_stream(stream);
                            match event_sender.send_async(ServerEvent::Socket(socket)).await {
                                Err(..) => break,
                                Ok(..) => (),
                            }
                        }
                        Err(e) => match event_sender.send_async(ServerEvent::IoError(e)).await {
                            Err(..) => break,
                            Ok(..) => (),
                        },
                    }
                }
            });
        }
        #[cfg(not(feature = "async"))]
        {
            std::thread::spawn(move || loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let socket = Socket::<P>::from_tcp_stream(stream);
                        match event_sender.send(ServerEvent::Socket(socket)) {
                            Err(..) => break,
                            Ok(..) => (),
                        }
                    }
                    Err(e) => match event_sender.send(ServerEvent::IoError(e)) {
                        Err(..) => break,
                        Ok(..) => (),
                    },
                }
            });
        }

        Self { event_receiver }
    }

    /// Returns a reference to the [`ServerEvent`] receiver flume channel.
    /// Use this to receive events from the [`Server`], a.k.a. new sockets and errors.
    ///
    /// See type-level documentation for usage.
    pub fn event_receiver(&self) -> &Receiver<ServerEvent<P>> {
        &self.event_receiver
    }
}
