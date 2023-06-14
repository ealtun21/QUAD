use clap::{Args, Parser, Subcommand};

/// Quick UDP File Sharing
#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about,
    long_about = "Quick UDP File Sharing is a simple p2p file sharing application."
)]
pub struct QuardArgs {
    #[clap(subcommand)]
    pub quad_type: QuadType,
}

#[derive(Debug, Subcommand)]
pub enum QuadType {
    /// The server-mode of QUAD to be able to holepunch NAT.
    Helper(Helper),
    /// The client-mode of QUAD to send files p2p
    Send(Sender),
    /// The client-mode of QUAD to receiver files p2p
    Receive(Receiver),
}

#[derive(Debug, Args)]
pub struct Helper {
    /// The port to bind for the server
    pub port: u16,
}

#[derive(Debug, Args)]
pub struct Sender {
    /// Unique identifier
    pub identifier: String,

    /// File path
    pub input: String,

    /// URL to the helper
    #[clap(default_value = "nyverin.com:4277")]
    pub address: String,

    /// Bitrate (lower = more reliable, higher = faster)
    #[clap(default_value = "256")]
    pub bitrate: u64,

    /// Start position
    #[clap(default_value = "0")]
    pub start_position: u64,
}

#[derive(Debug, Args)]
pub struct Receiver {
    /// Unique identifier
    pub identifier: String,

    /// File path
    pub output: String,

    /// URL to the helper
    #[clap(default_value = "nyverin.com:4277")]
    pub address: String,

    /// Bitrate (lower = more reliable, higher = faster)
    #[clap(default_value = "256")]
    pub bitrate: u64,

    /// Start position
    #[clap(default_value = "0")]
    pub start_position: u64,
}
