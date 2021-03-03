use taper::{Server, ServerEvent, SocketEvent};

pub fn main() {
    // Try to bind server for listening on localhost and port 1234 using u32 packets
    let server = Server::<u32, u32>::bind("127.0.0.1:1234").expect("Could not bind the server");

    // Wait for the connection of a single socket
    let socket = match server
        .event_receiver()
        .recv()
        .expect("Could not receive server event")
    {
        ServerEvent::Socket(socket) => socket,
        ServerEvent::IoError(e) => panic!(e),
    };

    let packet = match socket
        .event_receiver()
        .recv()
        .expect("Could not receive socket event")
    {
        SocketEvent::Packet(packet) => packet,
        SocketEvent::InvalidPacket => panic!("Received an invalid packet"),
        SocketEvent::IoError(e) => panic!(e),
    };

    // Print the first packet received
    // With the client example below this would print 10
    println!("Received {} from the remote socket !", packet);

    // Reply to the socket
    // 11 with the client example
    socket
        .packet_sender()
        .send(packet + 1)
        .expect("Could not respond to client");
}
