use std::{collections::HashMap, io::Cursor, net::SocketAddr, sync::Arc};

use anyhow::anyhow;
use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use nbt::{NamedTag, NBT};
use protocol::{packet::PacketBuilder, varint::VarInt};
use serde::{Deserialize, Serialize};
use surrealdb::{engine::local::RocksDb, RecordId, Surreal};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_byteorder::{AsyncReadBytesExt, BigEndian};

pub mod login;
pub mod nbt;
pub mod protocol;
pub struct Context {
    db: Surreal<surrealdb::engine::local::Db>,
}

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    name: String,
    hash: String,
}

#[derive(Debug, Deserialize)]
struct Record {
    #[allow(dead_code)]
    id: RecordId,
}

impl Context {
    pub async fn player_exists(&self, name: &str) -> Result<bool> {
        let users: Vec<Credentials> = self.db.select("credentials").await?;
        let user = users.iter().find(|a| a.name == name);
        Ok(user.is_some())
    }

    pub async fn register(&self, name: &str, password: &str) -> Result<bool> {
        if self.player_exists(&name).await? {
            return Ok(false);
        }

        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        let hash = argon2.hash_password(password.as_bytes(), &salt)?;
        let hash = hash.serialize().to_string();

        let _: Option<Record> = self
            .db
            .create("credentials")
            .content(Credentials {
                name: name.to_string(),
                hash,
            })
            .await?;

        Ok(true)
    }

    pub async fn authenticate(&self, name: &str, password: &str) -> Result<bool> {
        if !self.player_exists(&name).await? {
            return Ok(false);
        }

        let argon2 = Argon2::default();

        let users: Vec<Credentials> = self.db.select("credentials").await?;
        let user = users.iter().find(|a| a.name == name);

        if let Some(user) = user {
            let hash = PasswordHash::new(&user.hash)?;

            if argon2.verify_password(password.as_bytes(), &hash).is_ok() {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub struct State {
    state: i32,
    peer: Option<SocketAddr>,
    username: Option<String>,
    context: Arc<Mutex<Context>>,
}

impl State {
    pub fn new(context: Arc<Mutex<Context>>) -> Self {
        State {
            state: 0,
            peer: None,
            username: None,
            context,
        }
    }

    pub async fn receive_packet(&mut self, stream: &mut TcpStream) -> Result<()> {
        // println!("Receiving packet");
        let (packet_id, buffer) = protocol::read_generic_packet(stream).await?;
        let mut buffer = Cursor::new(buffer);
        // println!("Received packet with ID: {:02x}", packet_id);

        match self.state {
            0 => match packet_id {
                0 => {
                    let protocol_version = VarInt::read(&mut buffer).await?.into_inner();
                    let server_address = protocol::read_string(&mut buffer).await?;
                    let server_port = buffer.read_u16::<BigEndian>().await?;
                    let next_state = VarInt::read(&mut buffer).await?.into_inner();

                    println!("Protocol version: {}", protocol_version);
                    println!("Server address: {}", server_address);
                    println!("Server port: {}", server_port);
                    println!("Next state: {}", next_state);

                    self.state = next_state;
                }
                _ => {
                    println!("Handshake: Unknown packet ID");
                }
            },
            1 => match packet_id {
                0 => {
                    println!("Status request");

                    let payload = include_str!("status_response.json");

                    let response = PacketBuilder::new(0x00).with_string(payload).build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;
                }
                1 => {
                    let payload = buffer.read_i64::<BigEndian>().await?;

                    stream
                        .write_all(&PacketBuilder::new(0x01).with_i64(payload).build())
                        .await?;
                    stream.flush().await?;
                }
                _ => {
                    println!("Status: Unknown packet ID");
                }
            },
            2 => match packet_id {
                0 => {
                    let username = protocol::read_string(&mut buffer).await?;
                    // let uuid = buffer.read_u128::<BigEndian>().await?;
                    println!("Login request: {}", username);

                    self.username = Some(username.clone());

                    // Send login success

                    let response = PacketBuilder::new(0x02)
                        .with_uuid(0)
                        .with_string(&username)
                        .with_var_int(0)
                        // .with_bool(false) 1.20.5+
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    let registry_codec = nbt::from_json(include_str!("registry_codec.json"));

                    let response = PacketBuilder::new(0x25)
                        .with_i32(0) // entity id
                        .with_bool(false) // is hardcore
                        .with_u8(3) // gamemode
                        .with_u8(0xff) // previous gamemode
                        .with_var_int(1) // dim count
                        .with_string("minecraft:the_end") // dim name
                        .with_nbt(&registry_codec)
                        // .with_raw_bytes(&[0x0a, 0x00, 0x00, 0x00]) // empty NBT
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
                        // .with_string("minecraft:the_end") // world name
                        // .with_position(0, 0, 0)
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    println!("Login success");

                    self.state = 3;

                    // Send slot select
                    let response = PacketBuilder::new(0x4a)
                        .with_u8(0) // slot index
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    // Send update recipes
                    let response = PacketBuilder::new(0x6a)
                        .with_var_int(0) // recipe count
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    // Send update tags
                    let response = PacketBuilder::new(0x6b)
                        .with_var_int(0) // count
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    // Send entity event
                    let response = PacketBuilder::new(0x1a)
                        .with_i32(0) // entity id
                        .with_u8(28) // value
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

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

                    // Send empty player info
                    let response = PacketBuilder::new(0x37)
                        .with_var_int(0) // action
                        .with_var_int(0) // player count
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    // Send set center chunk
                    let response = PacketBuilder::new(0x4b)
                        .with_var_int(0) // x
                        .with_var_int(0) // z
                        .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    println!("Sending chunks");

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

                    match self.context.lock().await.player_exists(&username).await {
                        Ok(b) => match b {
                            false => {
                                println!("Player nx");
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
                            eprintln!("DB ERROR: {:?}", e);

                            return self
                                .kick(stream, "Database error. Please contact one of the admins.")
                                .await;
                        }
                    }

                    // let response = PacketBuilder::new(0x5d)
                    //     .with_string("{\"text\":\"Please log in.\"}")
                    //     .build();

                    stream.write_all(&response).await?;
                    stream.flush().await?;

                    println!("Finished login sequence");
                }
                _ => {
                    println!("Login: Unknown packet ID");
                }
            },
            3 => {
                match packet_id {
                    0x20 => {
                        println!("Keepalive C2S");
                        let payload = buffer.read_i32::<BigEndian>().await?;

                        stream
                            .write_all(&PacketBuilder::new(0x2f).with_i32(payload).build())
                            .await?;
                        stream.flush().await?;
                    }
                    0x12 => {
                        println!("Keepalive C2S (long)");
                        let payload = buffer.read_i64::<BigEndian>().await?;

                        stream
                            .write_all(&PacketBuilder::new(0x20).with_i64(payload).build())
                            .await?;
                        stream.flush().await?;
                    }
                    0x5 => {
                        let message = protocol::read_string(&mut buffer).await?;
                        match &self.username {
                            Some(username) => println!("{username}: {message}"),
                            None => println!("<unknown>: {message}"),
                        }
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
                                let Some(ref username) = self.username else {
                                    return self.kick(stream, "Internal error.").await;
                                };

                                match self.context.lock().await.authenticate(username, password).await {
                                    Ok(success) => match success {
                                        false => {
                                            return self
                                                .kick(stream, "Invalid password or user not registered.")
                                                .await;
                                        }
                                        true => {
                                            println!("Login successful for {}", username);
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
                                        eprintln!("DB ERROR: {:?}", e);

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

                                let Some(ref username) = self.username else {
                                    return self.kick(stream, "Internal error.").await;
                                };

                                match self.context.lock().await.register(username, password).await {
                                    Ok(success) => match success {
                                        false => {
                                            return self
                                                .kick(stream, "This user is already registered.")
                                                .await;
                                        }
                                        true => {
                                            println!("Registration successful for {}", username);
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
                                        eprintln!("DB ERROR: {:?}", e);

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
                    _ => {
                        // println!("Play: Unknown packet ID {packet_id:02x}");
                    }
                }
            }
            _ => {
                println!("Unknown state");
            }
        }

        Ok(())
    }

    pub async fn kick(& self, stream: &mut TcpStream, reason: impl Into<String>) -> Result<()> {
        let reason = reason.into();
        let response = PacketBuilder::new(0x19)
            .with_string(&format!("{{\"text\":\"{reason}\"}}"))
            .build();

        stream.write_all(&response).await?;
        stream.flush().await?;

        return Err(anyhow!("Kicked player {:?} with reason {}", self.username, reason))
    }

    pub async fn connect(mut self, mut stream: tokio::net::TcpStream, peer: SocketAddr) {
        self.peer = Some(peer);
        loop {
            match self.receive_packet(&mut stream).await {
                Ok(_) => {}
                Err(e) => {
                    println!("Error: {:?}", e);
                    break;
                }
            }
            if self.state == -1 {
                break;
            }
        }
        println!("Connection closed");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:30067").await?;
    let context = Context {
        db: login::init_db().await?,
    };
    let context = Arc::new(Mutex::new(context));

    loop {
        let (socket, peer) = listener.accept().await?;

        println!("Accepted connection from: {}", socket.peer_addr()?);

        let state = State::new(Arc::clone(&context));
        tokio::spawn(async move {
            state.connect(socket, peer).await;
        });
    }
}
