# Taper

Taper is a Rust library made for easy packet based network communication.

## Async

Taper supports async usage with the `async` feature.
When enabled, taper will use Tokio to execute concurrently all packet listener.
If disabled, OS Threads are used instead

While this may be overkill for Clients, this may be a great performance improvement for Servers.

## Server Usage

```rust
use taper::Server;

// Try to bind server for listening on localhost and port 1234
// Using u32 packets
let server = Server::<u32>::bind("127.0.0.1:1234").unwrap();

// Wait for the connection of a single socket
let socket = server.event_receiver().recv().unwrap();

// Do whatever you want with the socket!
```

## Client Usage

```rust
use taper::{Socket, SocketEvent};

let socket = Socket::<u32>::connect("127.0.0.1:1234").unwrap();

socket.packet_sender().send(56745).unwrap();

loop {
    match socket.event_receiver().recv().unwrap() {
        SocketEvent::Packet(packet) => println!("Received a packet from the server: {}", packet),
        SocketEvent::InvalidPacket => println!("The server sent an invalid packet :("),
        SocketEvent::IoError(error) => println!("An error occurred {}", error),
    }
}
```

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](https://choosealicense.com/licenses/mit/)
