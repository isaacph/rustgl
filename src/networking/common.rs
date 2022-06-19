use std::{net::{UdpSocket, SocketAddr}, collections::HashMap};

pub fn udp_recv_all(socket: &UdpSocket, buffer: &mut [u8], limit: Option<usize>)
-> (HashMap<SocketAddr, Vec<Box<[u8]>>>, Option<std::io::Error>) {
let mut error = None;
let mut map: HashMap<SocketAddr, Vec<Box<[u8]>>> = HashMap::new();
let limit = match limit {
    Some(limit) => limit,
    None => usize::MAX
};
for _ in 0..limit {
    match socket.recv_from(buffer) {
        Ok((sent, addr)) => {
            let packet = Vec::from(&buffer[0..sent]).into_boxed_slice();
            let spot = map.get_mut(&addr);
            match spot {
                Some(spot) => {
                    spot.push(packet);
                },
                None => {
                    map.insert(addr, vec![packet]);
                }
            }
        },
        Err(err) => {
            error = Some(err);
            break
        }
    }
}
(map, error)
}