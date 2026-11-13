mod socket;
#[cfg(feature = "_rt-tokio")]
pub mod tcp_connector;
pub mod tls;

pub use socket::{
    connect_tcp, connect_uds, BufferedSocket, Socket, SocketIntoBox, WithSocket, WriteBuffer,
};
