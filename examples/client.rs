use taper::{Socket, SocketEvent};

pub fn main() {
    // Connect to localhost server using u32 packets
    let socket =
        Socket::<u32, u32>::connect("127.0.0.1:1234").expect("Could not connect to local server");

    // Send the value '10'
    socket
        .packet_sender()
        .send(10)
        .expect("Could not send the packet");

    // Wait for a response packet
    let response_packet = match socket
        .event_receiver()
        .recv()
        .expect("Could not receive an event")
    {
        SocketEvent::Packet(packet) => packet,
        SocketEvent::InvalidPacket => panic!("Received an invalid packet"),
        SocketEvent::IoError(e) => panic!("IoError: {}", e),
    };

    // With the server example above this would be '11'
    println!("Received the value '{}' from the server", response_packet);
}
