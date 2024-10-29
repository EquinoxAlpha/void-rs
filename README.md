# void-rs

Minimal Minecraft server (limbo) implementation in Rust, for authentication on a server that I am working on.

As of right now, the code is a complete mess, and not intended for external usage. If you are looking for a server implementation with relatively little dependencies to hack upon, then feel free to clone this repository.

* Supports Minecraft 1.19.2 clients (protocol version 760)
* Stores logins using SurrealDB

Needs to be ran behind a Velocity proxy with modern player information forwarding.
Please keep in mind that if you do want a minimal server implementation without Velocity support, you'll need to change the code to immediately start
sending the Login (play) packet when a client has logged in, instead of waiting to receive player information from the proxy.
Also strip out SurrealDB. It has a lot of dependencies.