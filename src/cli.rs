use clap::{Parser, Subcommand};

/// QUAD
#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = "Quick UDP File Sharing is a simple p2p file sharing application."
)]
pub struct Quad {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// The server-mode of QUAD to be able to holepunch NAT.
    Helper {
        /// Port number
        #[arg(short, long)]
        port: u16,
    },
    /// The client-mode of QUAD to send files p2p
    Sender {
        /// URL to the helper
        #[clap(default_value = "nyverin.com:4277")]
        #[arg(short, long)]
        address: String,

        /// Unique identifier
        #[arg(short, long)]
        unique_identifier: String,

        /// File path
        #[arg(short, long)]
        input: String,

        /// Bitrate (lower = more reliable, higher = faster)
        #[clap(default_value = "256")]
        #[arg(short, long)]
        bitrate: u64,

        /// Start position
        #[clap(default_value = "0")]
        #[arg(short, long)]
        start_position: u64,
    },
    /// The client-mode of QUAD to receiver files p2p
    Receiver {
        /// URL to the helper
        #[clap(default_value = "nyverin.com:4277")]
        #[arg(short, long)]
        address: String,

        /// Unique identifier
        #[arg(short, long)]
        unique_identifier: String,

        /// File path
        #[arg(short, long)]
        output: String,

        /// Bitrate (lower = more reliable, higher = faster)
        #[arg(short, long)]
        #[clap(default_value = "256")]
        bitrate: u64,

        /// Start position
        #[arg(short, long)]
        #[clap(default_value = "0")]
        start_position: u64,
    },
}
