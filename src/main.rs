use std::io::Read;
use std::{io::Cursor, net::SocketAddr, sync::Arc};
use anyhow::anyhow;
use anyhow::Result;
use nbt::{NamedTag, NBT};
use protocol::{packet::PacketBuilder, varint::VarInt};
use surrealdb::Surreal;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_byteorder::{AsyncReadBytesExt, BigEndian};

pub mod db;
pub mod nbt;
pub mod protocol;

pub struct Context {
    db: Surreal<surrealdb::engine::local::Db>,
}

pub struct State {
    state: i32,
    peer: SocketAddr,
    real_address: String,
    username: String,
    context: Arc<Mutex<Context>>,
    conn_id: i32,
}

impl State {
    pub fn new(context: Arc<Mutex<Context>>, peer: SocketAddr) -> Self {
        State {
            state: 0,
            peer,
            username: String::from("<name unknown>"),
            real_address: String::from("<IP address unknown>"),
            context,
            conn_id: rand::random(),
        }
    }

    pub async fn send_packet(
        &self,
        stream: &mut TcpStream,
        packet: impl Into<Vec<u8>>,
    ) -> anyhow::Result<()> {
        stream.write_all(&packet.into()).await?;
        stream.flush().await?;
        Ok(())
    }

    pub async fn receive_packet(&mut self, stream: &mut TcpStream) -> Result<()> {
        let (packet_id, buffer) = protocol::read_generic_packet(stream).await?;
        let mut buffer = Cursor::new(buffer);

        match self.state {
            0 => match packet_id {
                0 => {
                    let _protocol_version = VarInt::read(&mut buffer).await?.into_inner();
                    let _server_address = protocol::read_string(&mut buffer).await?;
                    let _server_port = buffer.read_u16::<BigEndian>().await?;
                    let next_state = VarInt::read(&mut buffer).await?.into_inner();

                    self.state = next_state;
                }
                _ => ()
            },
            1 => match packet_id {
                0 => {
                    let payload = include_str!("status_response.json");

                    let response = PacketBuilder::new(0x00).with_string(payload).build();

                    self.send_packet(stream, response).await?;
                }
                1 => {
                    let payload = buffer.read_i64::<BigEndian>().await?;

                    stream
                        .write_all(&PacketBuilder::new(0x01).with_i64(payload).build())
                        .await?;
                    stream.flush().await?;
                }
                _ => ()
            },
            2 => match packet_id {
                0 => {
                    let username = protocol::read_string(&mut buffer).await?;

                    self.username = username.clone();

                    let response = PacketBuilder::new(0x04)
                        .with_var_int(self.conn_id.abs())
                        .with_string("velocity:player_info")
                        .with_u8(1)
                        .build();

                    self.send_packet(stream, response).await?;
                }
                0x02 => {
                    let message_id = VarInt::read(&mut buffer).await?;

                    match buffer.read_u8().await? {
                        1 => {
                            let mut signature = vec![0u8; 32];
                            buffer.read_exact(&mut signature)?;

                            let version = VarInt::read(&mut buffer).await?;
                            let address = protocol::read_string(&mut buffer).await?;
                            let uuid = buffer.read_u128::<BigEndian>().await?;
                            self.real_address = address;

                            let username = protocol::read_string(&mut buffer).await?;
                            self.username = username;
                            
                            let properties_len = VarInt::read(&mut buffer).await?;

                            for _ in 0..properties_len.into_inner() {
                                let name = protocol::read_string(&mut buffer).await?;
                                let value = protocol::read_string(&mut buffer).await?;
                                let has_signature = buffer.read_u8().await?;
                                if has_signature == 1 {
                                    let _signature = protocol::read_string(&mut buffer).await?;
                                }
                            }

                            if version.into_inner() == 2 {
                                let mut _ignored = vec![0u8; 8 + 512 + 4096];
                                buffer.read_exact(&mut signature)?;
                            }
                        }
                        _ => {
                            // this state should be almost impossible to reach.
                            // however, we all know what happens with supposedly unreachable code.
                            return Err(anyhow!("Raw connection from {:?}", self.peer))
                        }
                    }

                    // Proceed with normal login sequence

                    // Send login success

                    let response = PacketBuilder::new(0x02)
                        .with_uuid(0)
                        .with_string(&self.username)
                        .with_var_int(0)
                        .build();

                    self.send_packet(stream, response).await?;

                    let registry_codec = nbt::from_json(include_str!("registry_codec.json"));

                    let response = PacketBuilder::new(0x25)
                        .with_i32(0) // entity id
                        .with_bool(false) // is hardcore
                        .with_u8(3) // gamemode
                        .with_u8(0xff) // previous gamemode
                        .with_var_int(1) // dim count
                        .with_string("minecraft:the_end") // dim name
                        .with_nbt(&registry_codec)
                        .with_string("minecraft:the_end") // dimension type
                        .with_string("minecraft:the_end") // dimension name
                        .with_i64(0) // hashed (and truncated) seed
                        .with_var_int(20) // max players
                        .with_var_int(2) // view distance
                        .with_var_int(2) // simulation distance
                        .with_bool(false) // reduce debug info
                        .with_bool(false) // enable respawn screen
                        .with_bool(true) // is debug
                        .with_bool(false) // is flat
                        .with_bool(false) // has death location
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send slot select
                    let response = PacketBuilder::new(0x4a)
                        .with_u8(0) // slot index
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send update recipes
                    let response = PacketBuilder::new(0x6a)
                        .with_var_int(0) // recipe count
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send update tags
                    let response = PacketBuilder::new(0x6b)
                        .with_var_int(0) // count
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send entity event
                    let response = PacketBuilder::new(0x1a)
                        .with_i32(0) // entity id
                        .with_u8(28) // value
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send synchronize player position
                    let response = PacketBuilder::new(0x39)
                        .with_double(0.0) // x
                        .with_double(0.0) // y
                        .with_double(0.0) // z
                        .with_float(0.0) // yaw
                        .with_float(0.0) // pitch
                        .with_u8(0) // flags
                        .with_var_int(42) // teleport id
                        .with_bool(false) // dismount vehicle
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send empty player info
                    let response = PacketBuilder::new(0x37)
                        .with_var_int(0) // action
                        .with_var_int(0) // player count
                        .build();

                    self.send_packet(stream, response).await?;

                    // Send set center chunk
                    let response = PacketBuilder::new(0x4b)
                        .with_var_int(0) // x
                        .with_var_int(0) // z
                        .build();

                    self.send_packet(stream, response).await?;

                    // // Begin sending chunks

                    for x in 0..5 {
                        for z in 0..5 {
                            let mut data = vec![];
                            for _ in 0..24 {
                                data.extend_from_slice(&[
                                    00u8, 00, 00, 00, 00, 0x01, 0x02, 0x27, 0x03, 0x01, 0xCC, 0xFF,
                                    0xCC, 0xFF, 0xCC, 0xFF, 0xCC, 0xFF,
                                ]); // empty raw chunk, from wiki.vg
                            }
                            let response = PacketBuilder::new(0x21)
                                .with_i32(x - 2) // chunk x
                                .with_i32(z - 2) // chunk z
                                .with_nbt(&NamedTag::new(
                                    "",
                                    NBT::Compound(vec![NamedTag::new(
                                        "MOTION_BLOCKING",
                                        NBT::LongArray(vec![0; 36]),
                                    )]),
                                ))
                                .with_var_int(data.len() as _) // size of data
                                .with_raw_bytes(&data)
                                .with_var_int(0) // no. of block entities
                                .with_bool(true) // trust edges for light updates
                                .with_var_int(0) // bit set for sky light mask (length 0 = no data)
                                .with_var_int(0) // bit set for block light mask
                                .with_var_int(0) // bit set for empty sky light mask
                                .with_var_int(0) // bit set for empty block light mask
                                .with_var_int(0) // no. of sky lights
                                .with_var_int(0) // no. of block lights
                                .build();

                            stream.write_all(&response).await?;
                            stream.flush().await?;
                        }
                    }

                    // Send synchronize player position
                    let response = PacketBuilder::new(0x39)
                        .with_double(0.0) // x
                        .with_double(0.0) // y
                        .with_double(0.0) // z
                        .with_float(0.0) // yaw
                        .with_float(0.0) // pitch
                        .with_u8(0) // flags
                        .with_var_int(42) // teleport id
                        .with_bool(false) // dismount vehicle
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    log::info!("{} [{:?}] has connected to the login server.", self.username, self.peer);

                    match self.context.lock().await.player_exists(&self.username).await {
                        Ok(b) => match b {
                            false => {
                                let response = PacketBuilder::new(0x5d)
                                    .with_string("{\"text\":\"/register [password] [password]\"}")
                                    .build();

                                stream.write_all(&response).await?;
                                stream.flush().await?;
                            }
                            true => {
                                let response = PacketBuilder::new(0x5d)
                                    .with_string("{\"text\":\"/login [password]\"}")
                                    .build();

                                stream.write_all(&response).await?;
                                stream.flush().await?;
                            }
                        },
                        Err(e) => {
                            log::error!("Database error: {:?}", e);

                            return self
                                .kick(stream, "Database error. Please contact one of the admins.")
                                .await;
                        }
                    }

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    // Switch over to the "play" state
                    self.state = 3;
                }
                _ => ()
            },
            3 => {
                match packet_id {
                    0x20 => {
                        let payload = buffer.read_i32::<BigEndian>().await?;

                        stream
                            .write_all(&PacketBuilder::new(0x2f).with_i32(payload).build())
                            .await?;
                        stream.flush().await?;
                    }
                    0x12 => {
                        let payload = buffer.read_i64::<BigEndian>().await?;

                        stream
                            .write_all(&PacketBuilder::new(0x20).with_i64(payload).build())
                            .await?;
                        stream.flush().await?;
                    }
                    0x4 => {
                        let command = protocol::read_string(&mut buffer).await?;
                        let args = command.split(" ").collect::<Vec<&str>>();
                        let command = args[0];

                        match command {
                            "login" => {
                                if args.len() != 2 {
                                    return self
                                        .kick(stream, "Invalid syntax. Usage: /login [password]")
                                        .await;
                                }

                                let password = args[1];

                                match self
                                    .context
                                    .lock()
                                    .await
                                    .authenticate(&self.username, password)
                                    .await
                                {
                                    Ok(success) => match success {
                                        false => {
                                            log::warn!("{} [{:?}] has specified an incorrect password.", self.username, self.peer);
                                            return self
                                                .kick(
                                                    stream,
                                                    "Invalid password or user not registered.",
                                                )
                                                .await;
                                        }
                                        true => {
                                            log::info!("{} [{:?}] has successfully authenticated.", self.username, self.peer);

                                            stream
                                                .write_all(
                                                    &PacketBuilder::new(0x16)
                                                        .with_string("BungeeCord")
                                                        .with_raw_bytes(b"\x00\x07Connect")
                                                        .with_raw_bytes(b"\x00\x04main")
                                                        .build(),
                                                )
                                                .await?;
                                            stream.flush().await?;
                                        }
                                    },
                                    Err(e) => {
                                        log::error!("Database error: {:?}", e);

                                        return self
                                            .kick(
                                                stream,
                                                "Database error. Please contact one of the admins.",
                                            )
                                            .await;
                                    }
                                }
                            }
                            "register" => {
                                if args.len() != 3 {
                                    return self.kick(stream, "Invalid syntax. Usage: /register [password] [password]").await;
                                }

                                let password = args[1];
                                if args[1] != args[2] {
                                    if args.len() != 2 {
                                        return self.kick(stream, "Passwords do not match.").await;
                                    }
                                }

                                match self.context.lock().await.register(&self.username, password).await {
                                    Ok(success) => match success {
                                        false => {
                                            log::warn!("{} [{:?}] attempted double registration.", self.username, self.peer);
                                            return self
                                                .kick(stream, "This user is already registered.")
                                                .await;
                                        }
                                        true => {
                                            log::info!("{} [{:?}] has successfully registered.", self.username, self.peer);
                                            stream
                                                .write_all(
                                                    &PacketBuilder::new(0x16)
                                                        .with_string("BungeeCord")
                                                        .with_raw_bytes(b"\x00\x07Connect")
                                                        .with_raw_bytes(b"\x00\x04main")
                                                        .build(),
                                                )
                                                .await?;
                                            stream.flush().await?;
                                        }
                                    },
                                    Err(e) => {
                                        log::error!("Database error: {:?}", e);

                                        return self
                                            .kick(
                                                stream,
                                                "Database error. Please contact one of the admins.",
                                            )
                                            .await;
                                    }
                                }
                            }
                            _ => {
                                return self.kick(stream, "Invalid command.").await;
                            }
                        }
                    }
                    _ => ()
                }
            }
            _ => {
                return Err(anyhow!("Unknown connection state."))
            }
        }

        Ok(())
    }

    pub async fn kick(&self, stream: &mut TcpStream, reason: impl Into<String>) -> Result<()> {
        let reason = reason.into();
        let response = PacketBuilder::new(0x19)
            .with_string(&format!("{{\"text\":\"{reason}\"}}"))
            .build();

        stream.write_all(&response).await?;
        stream.flush().await?;

        return Err(anyhow!(
            "Kicked player {} [{:?}] with reason: \"{}\"",
            self.username,
            self.peer,
            reason
        ));
    }

    pub async fn connect(mut self, mut stream: tokio::net::TcpStream) {
        loop {
            match self.receive_packet(&mut stream).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("{:?}", e);
                    break;
                }
            }
            if self.state == -1 {
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    simplelog::TermLogger::init(
        log::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )?;

    let socket = match std::env::args().nth(1) {
        Some(socket) => socket,
        None => {
            eprintln!("You must specify an address and port.");
            eprintln!("Usage: ./void-rs [ip:port]");
            return Err(anyhow!("unspecified socket address"));
        }
    };

    let listener = TcpListener::bind(socket).await?;
    let context = Context {
        db: db::init_db().await?,
    };
    let context = Arc::new(Mutex::new(context));

    loop {
        let (socket, peer) = listener.accept().await?;

        log::debug!("Accepted connection from: {}", socket.peer_addr()?);

        let state = State::new(Arc::clone(&context), peer);
        tokio::spawn(async move {
            state.connect(socket).await;
        });
    }
}
