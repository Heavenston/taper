#[cfg(not(feature = "async"))]
use std::net;
#[cfg(feature = "async")]
use tokio::net;

use crate::Socket;
use flume::Receiver;
use net::TcpListener;
use serde::{de::DeserializeOwned, Serialize};

pub enum ServerEvent<P>
where
    P: Serialize + Send + 'static,
{
    Socket(Socket<P>),
    IoError(std::io::Error),
}

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
    #[cfg(feature = "async")]
    pub async fn bind_async(addr: impl net::ToSocketAddrs) -> Result<Self, std::io::Error> {
        let tcp_listener = TcpListener::bind(addr).await?;
        Ok(Self::from_tcp_listener(tcp_listener))
    }
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

    pub fn event_receiver(&self) -> &Receiver<ServerEvent<P>> {
        &self.event_receiver
    }
}
