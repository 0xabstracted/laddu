use clap::{Parser, Subcommand};

use crate::constants::{DEFAULT_ASSETS, DEFAULT_CACHE, DEFAULT_CONFIG};

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    /// Log level: trace, debug, info, warn, error, off
    #[clap(short, long, global = true)]
    pub log_level: Option<String>,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Interactive process to create the config file
    CreateConfig {
        /// Path to the config file
        #[clap(short, long)]
        config: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the keypair file [default: solana config or "~/.config/solana/id.json"]
        #[clap(short, long)]
        keypair: Option<String>,

        /// Path to the directory with the assets
        #[clap(default_value = DEFAULT_ASSETS)]
        assets_dir: String,
    },
    /// Create a magic hat deployment from assets
    Launch {
        /// Path to the directory with the assets to upload
        #[clap(default_value = DEFAULT_ASSETS)]
        assets_dir: String,

        /// Path to the keypair file [default: solana config or "~/.config/solana/id.json"]
        #[clap(short, long)]
        keypair: Option<String>,

        /// Path to the config file
        #[clap(short, long, default_value = DEFAULT_CONFIG)]
        config: String,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,

        /// Strict mode: validate against JSON metadata standard exactly
        #[clap(long)]
        strict: bool,
    },
    /// Mint one NFT from magic hat
    Mint {
        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,

        /// Amount of NFTs to be minted in bulk
        #[clap(short, long)]
        number: Option<u64>,

        /// Address of magic hat to mint from.
        #[clap(long)]
        magic_hat: Option<String>,
    },

    /// Update the magic hat config on-chain
    Update {
        /// Path to the config file, defaults to "config.json"
        #[clap(short, long, default_value = DEFAULT_CONFIG)]
        config: String,

        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,

        /// Pubkey for the new authority
        #[clap(short, long)]
        new_authority: Option<String>,

        /// Address of magic hat to update.
        #[clap(long)]
        magic_hat: Option<String>,
    },

    /// Deploy cache items into magic hat config on-chain
    Deploy {
        /// Path to the config file, defaults to "config.json"
        #[clap(short, long, default_value = DEFAULT_CONFIG)]
        config: String,

        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,
    },

    /// Upload assets to storage and creates the cache config
    Upload {
        /// Path to the directory with the assets to upload
        #[clap(default_value = DEFAULT_ASSETS)]
        assets_dir: String,

        /// Path to the config file
        #[clap(short, long, default_value = DEFAULT_CONFIG)]
        config: String,

        /// Path to the keypair file [default: solana config or "~/.config/solana/id.json"]
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,
    },

    /// Withdraw funds from magic hat account closing it
    Withdraw {
        /// Address of magic hat to withdraw funds from.
        #[clap(long)]
        magic_hat: Option<String>,

        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// List available magic hats, no withdraw performed
        #[clap(long)]
        list: bool,
    },

    /// Validate JSON metadata files
    Validate {
        /// Assets directory to upload, defaults to "assets"
        #[clap(default_value = DEFAULT_ASSETS)]
        assets_dir: String,

        /// Strict mode: validate against JSON metadata standard exactly
        #[clap(long)]
        strict: bool,
    },

    /// Verify uploaded data
    Verify {
        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,
    },

    /// Show the on-chain config of an existing magic hat
    Show {
        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,

        /// Address of magic hat
        magic_hat: Option<String>,
    },

    /// Interact with the bundlr network
    Bundlr {
        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        #[clap(subcommand)]
        action: BundlrAction,
    },

    /// Manage the collection on the magic hat
    Collection {
        #[clap(subcommand)]
        command: CollectionSubcommands,
    },
}

#[derive(Subcommand)]
pub enum CollectionSubcommands {
    /// Set the collection mint on the magic hat
    Set {
        /// Address of collection mint to set the magic hat to.
        #[clap(long)]
        collection_mint: String,

        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,

        /// Address of magic hat to update.
        #[clap(long)]
        magic_hat: Option<String>,
    },

    /// Remove the collection from the magic hat
    Remove {
        /// Path to the keypair file, uses Sol config or defaults to "~/.config/solana/id.json"
        #[clap(short, long)]
        keypair: Option<String>,

        /// RPC Url
        #[clap(short, long)]
        rpc_url: Option<String>,

        /// Path to the cache file, defaults to "cache.json"
        #[clap(long, default_value = DEFAULT_CACHE)]
        cache: String,

        /// Address of magic hat to update.
        #[clap(long)]
        magic_hat: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum BundlrAction {
    /// Retrieve the balance on bundlr
    Balance,
    /// Withdraw funds from bundlr
    Withdraw,
}
