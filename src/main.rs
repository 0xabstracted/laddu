use std::{
    fs::OpenOptions,
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};
use clap::Parser;
use console::style;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{self, filter::LevelFilter, prelude::*, EnvFilter};

use laddu_cli::bundlr::{process_bundlr, BundlrArgs};
use laddu_cli::cli::{Cli, CollectionSubcommands, Commands};
use laddu_cli::collections::{
    process_remove_collection, process_set_collection, RemoveCollectionArgs, SetCollectionArgs,
};
use laddu_cli::constants::{COMPLETE_EMOJI, ERROR_EMOJI};
use laddu_cli::create_config::{process_create_config, CreateConfigArgs};
use laddu_cli::deploy::{process_deploy, DeployArgs};
use laddu_cli::launch::{process_launch, LaunchArgs};
use laddu_cli::mint::{process_mint, MintArgs};
use laddu_cli::show::{process_show, ShowArgs};
use laddu_cli::update::{process_update, UpdateArgs};
use laddu_cli::upload::{process_upload, UploadArgs};
use laddu_cli::validate::{process_validate, ValidateArgs};
use laddu_cli::verify::{process_verify, VerifyArgs};
use laddu_cli::withdraw::{process_withdraw, WithdrawArgs};

fn setup_logging(level: Option<EnvFilter>) -> Result<()> {
    // Log path; change this to be dynamic for multiple OSes.
    // Log in current directory for now.
    let log_path = PathBuf::from("laddu.log");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&log_path)
        .unwrap();

    // Prioritize user-provided level, otherwise read from RUST_LOG env var for log level, fall back to "tracing" if not set.
    let env_filter = if let Some(filter) = level {
        filter
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("trace"))
    };

    let formatting_layer = BunyanFormattingLayer::new("laddu".into(), file);
    let level_filter = LevelFilter::from_str(&env_filter.to_string())?;

    let subscriber = tracing_subscriber::registry()
        .with(formatting_layer.with_filter(level_filter))
        .with(JsonStorageLayer);

    set_global_default(subscriber).expect("Failed to set global default subscriber");

    Ok(())
}

#[tokio::main(worker_threads = 4)]
async fn main() {
    match run().await {
        Ok(()) => {
            println!(
                "\n{}{}",
                COMPLETE_EMOJI,
                style("Command successful.").green().bold().dim()
            );
        }
        Err(err) => {
            println!(
                "\n{}{} {}",
                ERROR_EMOJI,
                style("Error running command (re-run needed):").red(),
                err,
            );
            // finished the program with an error code to the OS
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    let log_level_error: Result<()> = Err(anyhow!(
        "Invalid log level: {:?}.\n Valid levels are: trace, debug, info, warn, error.",
        cli.log_level
    ));

    if let Some(user_filter) = cli.log_level {
        let filter = match EnvFilter::from_str(&user_filter) {
            Ok(filter) => filter,
            Err(_) => return log_level_error,
        };
        setup_logging(Some(filter))?;
    } else {
        setup_logging(None)?;
    }

    tracing::info!("Lend me some laddu, I am your neighbor.");

    let interrupted = Arc::new(AtomicBool::new(true));
    let ctrl_handler = interrupted.clone();

    ctrlc::set_handler(move || {
        if ctrl_handler.load(Ordering::SeqCst) {
            // we really need to exit
            println!(
                "\n\n{}{} Operation aborted.",
                ERROR_EMOJI,
                style("Error running command (re-run needed):").red(),
            );
            // finished the program with an error code to the OS
            std::process::exit(1);
        }
        // signal that we want to exit
        ctrl_handler.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    match cli.command {
        Commands::CreateConfig {
            config,
            keypair,
            rpc_url,
            assets_dir,
        } => process_create_config(CreateConfigArgs {
            config,
            keypair,
            rpc_url,
            assets_dir,
        })?,
        Commands::Launch {
            assets_dir,
            config,
            keypair,
            rpc_url,
            cache,
            strict,
        } => {
            process_launch(LaunchArgs {
                assets_dir,
                config,
                keypair,
                rpc_url,
                cache,
                strict,
                interrupted: interrupted.clone(),
            })
            .await?
        }
        Commands::Mint {
            keypair,
            rpc_url,
            cache,
            number,
            magic_hat,
        } => process_mint(MintArgs {
            keypair,
            rpc_url,
            cache,
            number,
            magic_hat,
        })?,
        Commands::Update {
            config,
            keypair,
            rpc_url,
            cache,
            new_authority,
            magic_hat,
        } => process_update(UpdateArgs {
            config,
            keypair,
            rpc_url,
            cache,
            new_authority,
            magic_hat,
        })?,
        Commands::Deploy {
            config,
            keypair,
            rpc_url,
            cache,
        } => {
            process_deploy(DeployArgs {
                config,
                keypair,
                rpc_url,
                cache,
                interrupted: interrupted.clone(),
            })
            .await?
        }
        Commands::Upload {
            assets_dir,
            config,
            keypair,
            rpc_url,
            cache,
        } => {
            process_upload(UploadArgs {
                assets_dir,
                config,
                keypair,
                rpc_url,
                cache,
                interrupted: interrupted.clone(),
            })
            .await?
        }
        Commands::Validate { assets_dir, strict } => {
            process_validate(ValidateArgs { assets_dir, strict })?
        }
        Commands::Withdraw {
            magic_hat,
            keypair,
            rpc_url,
            list,
        } => process_withdraw(WithdrawArgs {
            magic_hat,
            keypair,
            rpc_url,
            list,
        })?,
        Commands::Verify {
            keypair,
            rpc_url,
            cache,
        } => process_verify(VerifyArgs {
            keypair,
            rpc_url,
            cache,
        })?,
        Commands::Show {
            keypair,
            rpc_url,
            cache,
            magic_hat,
        } => process_show(ShowArgs {
            keypair,
            rpc_url,
            cache,
            magic_hat,
        })?,
        Commands::Collection { command } => match command {
            CollectionSubcommands::Set {
                collection_mint,
                keypair,
                rpc_url,
                cache,
                magic_hat,
            } => process_set_collection(SetCollectionArgs {
                collection_mint,
                keypair,
                rpc_url,
                cache,
                magic_hat,
            })?,
            CollectionSubcommands::Remove {
                keypair,
                rpc_url,
                cache,
                magic_hat,
            } => process_remove_collection(RemoveCollectionArgs {
                keypair,
                rpc_url,
                cache,
                magic_hat,
            })?,
        },
        Commands::Bundlr {
            keypair,
            rpc_url,
            action,
        } => {
            process_bundlr(BundlrArgs {
                keypair,
                rpc_url,
                action,
            })
            .await?
        }
    }

    Ok(())
}
