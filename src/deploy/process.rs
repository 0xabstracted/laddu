use anchor_client::solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction, system_program, sysvar,
};
use anchor_lang::prelude::AccountMeta;
use anyhow::Result;
use console::style;
use futures::future::select_all;
use rand::rngs::OsRng;
use solana_program::native_token::LAMPORTS_PER_SOL;
use spl_associated_token_account::get_associated_token_address;
use std::{
    cmp,
    collections::HashSet,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use magic_hat::accounts as nft_accounts;
use magic_hat::instruction as nft_instruction;
use magic_hat::{ConfigLine, Creator as MagicHatCreator, MagicHatData};
pub use mpl_token_metadata::state::{
    MAX_CREATOR_LIMIT, MAX_NAME_LENGTH, MAX_SYMBOL_LENGTH, MAX_URI_LENGTH,
};

use crate::cache::*;
use crate::common::*;
use crate::config::{data::*, parser::get_config_data};
use crate::deploy::data::*;
use crate::deploy::errors::*;
use crate::magic_hat::{parse_config_price, MAGIC_HAT_ID};
use crate::setup::{laddu_setup, setup_client};
use crate::utils::*;
use crate::validate::parser::{check_name, check_seller_fee_basis_points, check_symbol, check_url};

/// The maximum config line bytes per transaction.
const MAX_TRANSACTION_BYTES: usize = 1000;

/// The maximum number of config lines per transaction.
const MAX_TRANSACTION_LINES: usize = 17;

struct TxInfo {
    magichat_pubkey: Pubkey,
    payer: Keypair,
    chunk: Vec<(u32, ConfigLine)>,
}

pub async fn process_deploy(args: DeployArgs) -> Result<()> {
    // loads the cache file (this needs to have been created by
    // the upload command)
    let mut cache = load_cache(&args.cache, false)?;

    if cache.items.0.is_empty() {
        println!(
            "{}",
            style("No cache items found - run 'upload' to create the cache file first.")
                .red()
                .bold()
        );

        // nothing else to do, just tell that the cache file was not found (or empty)
        return Err(CacheError::CacheFileNotFound(args.cache).into());
    }

    // checks that all metadata information are present and have the
    // correct length

    for (index, item) in &cache.items.0 {
        if item.name.is_empty() {
            return Err(DeployError::MissingName(index.to_string()).into());
        } else {
            check_name(&item.name)?;
        }

        if item.metadata_link.is_empty() {
            return Err(DeployError::MissingMetadataLink(index.to_string()).into());
        } else {
            check_url(&item.metadata_link)?;
        }
    }

    let laddu_config = Arc::new(laddu_setup(args.keypair, args.rpc_url)?);
    let client = setup_client(&laddu_config)?;
    let config_data = get_config_data(&args.config)?;

    let magic_hat_address = &cache.program.magic_hat;

    // checks the magic hat data

    let num_items = config_data.number;
    let hidden = config_data.hidden_settings.is_some();

    if num_items != (cache.items.0.len() as u64) {
        return Err(anyhow!(
            "Number of items ({}) do not match cache items ({})",
            num_items,
            cache.items.0.len()
        ));
    } else {
        check_symbol(&config_data.symbol)?;
        check_seller_fee_basis_points(config_data.seller_fee_basis_points)?;
    }

    let magichat_pubkey = if magic_hat_address.is_empty() {
        println!(
            "{} {}Creating Magic Hat",
            style(if hidden { "[1/1]" } else { "[1/2]" }).bold().dim(),
            MAGICHAT_EMOJI
        );
        info!("Magic Hat address is empty, creating new Magic Hat...");

        let spinner = spinner_with_style();
        spinner.set_message("Creating Magic Hat...");

        let magichat_keypair = Keypair::generate(&mut OsRng);
        let magichat_pubkey = magichat_keypair.pubkey();

        let uuid = DEFAULT_UUID.to_string();
        let magichat_data = create_magic_hat_data(&client, &config_data, uuid)?;
        let program = client.program(MAGIC_HAT_ID);

        let treasury_wallet = match config_data.spl_token {
            Some(spl_token) => {
                let spl_token_account_figured = if config_data.spl_token_account.is_some() {
                    config_data.spl_token_account
                } else {
                    Some(get_associated_token_address(&program.payer(), &spl_token))
                };

                if config_data.sol_treasury_account.is_some() {
                    return Err(anyhow!("If spl-token-account or spl-token is set then sol-treasury-account cannot be set"));
                }

                // validates the mint address of the token accepted as payment
                check_spl_token(&program, &spl_token.to_string())?;

                if let Some(token_account) = spl_token_account_figured {
                    // validates the spl token wallet to receive proceedings from SPL token payments
                    check_spl_token_account(&program, &token_account.to_string())?;
                    token_account
                } else {
                    return Err(anyhow!(
                        "If spl-token is set, spl-token-account must also be set"
                    ));
                }
            }
            None => match config_data.sol_treasury_account {
                Some(sol_treasury_account) => sol_treasury_account,
                None => laddu_config.keypair.pubkey(),
            },
        };

        // all good, let's create the magic hat

        let sig = initialize_magic_hat(
            &config_data,
            &magichat_keypair,
            magichat_data,
            treasury_wallet,
            program,
        )?;
        info!("Magic Hat initialized with sig: {}", sig);
        info!(
            "Magic Hat created with address: {}",
            &magichat_pubkey.to_string()
        );

        cache.program = CacheProgram::new_from_cm(&magichat_pubkey);
        cache.sync_file()?;

        spinner.finish_and_clear();

        magichat_pubkey
    } else {
        println!(
            "{} {}Loading Magic Hat",
            style(if hidden { "[1/1]" } else { "[1/2]" }).bold().dim(),
            MAGICHAT_EMOJI
        );

        match Pubkey::from_str(magic_hat_address) {
            Ok(pubkey) => pubkey,
            Err(_err) => {
                error!(
                    "Invalid Magic Hat address in cache file: {}!",
                    magic_hat_address
                );
                return Err(
                    CacheError::InvalidMagicHatAddress(magic_hat_address.to_string()).into(),
                );
            }
        }
    };

    println!("{} {}", style("Magic Hat ID:").bold(), magichat_pubkey);

    if !hidden {
        println!(
            "\n{} {}Writing config lines",
            style("[2/2]").bold().dim(),
            PAPER_EMOJI
        );

        let config_lines = generate_config_lines(num_items, &cache.items)?;

        if config_lines.is_empty() {
            println!("\nAll config lines deployed.");
        } else {
            // clear the interruption handler value ahead of the upload
            args.interrupted.store(false, Ordering::SeqCst);

            let errors = upload_config_lines(
                laddu_config,
                magichat_pubkey,
                &mut cache,
                config_lines,
                args.interrupted,
            )
            .await?;

            if !errors.is_empty() {
                let mut message = String::new();
                message.push_str(&format!(
                    "Failed to deploy all config lines, {0} error(s) occurred:",
                    errors.len()
                ));

                let mut unique = HashSet::new();

                for err in errors {
                    unique.insert(err.to_string());
                }

                for u in unique {
                    message.push_str(&style("\n=> ").dim().to_string());
                    message.push_str(&u);
                }

                return Err(DeployError::AddConfigLineFailed(message).into());
            }
        }
    } else {
        println!("\nMagic Hat with hidden settings deployed.");
    }

    Ok(())
}

/// Create the magic hat data struct.
fn create_magic_hat_data(
    client: &Client,
    config: &ConfigData,
    uuid: String,
) -> Result<MagicHatData> {
    let go_live_date = Some(go_live_date_as_timestamp(&config.go_live_date)?);

    let end_settings = config
        .end_settings
        .as_ref()
        .map(|s| s.into_magichat_format());

    let whitelist_mint_settings = config
        .whitelist_mint_settings
        .as_ref()
        .map(|s| s.into_magichat_format());

    let hidden_settings = config
        .hidden_settings
        .as_ref()
        .map(|s| s.into_magichat_format());

    let gatekeeper = config
        .gatekeeper
        .as_ref()
        .map(|gatekeeper| gatekeeper.into_magichat_format());

    let mut creators: Vec<MagicHatCreator> = Vec::new();
    let mut share = 0u32;

    for creator in &config.creators {
        let c = creator.into_magichat_format()?;
        share += c.share as u32;

        creators.push(c);
    }

    if creators.is_empty() || creators.len() > (MAX_CREATOR_LIMIT - 1) {
        return Err(anyhow!(
            "The number of creators must be between 1 and {}.",
            MAX_CREATOR_LIMIT - 1,
        ));
    }

    if share != 100 {
        return Err(anyhow!(
            "Creator(s) share must add up to 100, current total {}.",
            share,
        ));
    }

    let price = parse_config_price(client, config)?;

    let data = MagicHatData {
        uuid,
        price,
        symbol: config.symbol.clone(),
        seller_fee_basis_points: config.seller_fee_basis_points,
        max_supply: 0,
        is_mutable: config.is_mutable,
        retain_authority: config.retain_authority,
        go_live_date,
        end_settings,
        creators,
        whitelist_mint_settings,
        hidden_settings,
        items_available: config.number,
        gatekeeper,
    };

    Ok(data)
}

/// Determine the config lines that need to be uploaded.
fn generate_config_lines(
    num_items: u64,
    cache_items: &CacheItems,
) -> Result<Vec<Vec<(u32, ConfigLine)>>> {
    let mut config_lines: Vec<Vec<(u32, ConfigLine)>> = Vec::new();
    let mut current: Vec<(u32, ConfigLine)> = Vec::new();
    let mut tx_size = 0;

    for i in 0..num_items {
        let item = match cache_items.0.get(&i.to_string()) {
            Some(item) => item,
            None => {
                return Err(
                    DeployError::AddConfigLineFailed(format!("Missing cache item {}", i)).into(),
                );
            }
        };

        if item.on_chain {
            // if the current item is on-chain already, store the previous
            // items as a transaction since we cannot have gaps in the indices
            // to write the config lines
            if !current.is_empty() {
                config_lines.push(current);
                current = Vec::new();
                tx_size = 0;
            }
        } else {
            let config_line = item
                .into_config_line()
                .expect("Could not convert item to config line");

            let size = (2 * STRING_LEN_SIZE) + config_line.name.len() + config_line.uri.len();

            if (tx_size + size) > MAX_TRANSACTION_BYTES || current.len() == MAX_TRANSACTION_LINES {
                // we need a separate tx to not break the size limit
                config_lines.push(current);
                current = Vec::new();
                tx_size = 0;
            }

            tx_size += size;
            current.push((i as u32, config_line));
        }
    }
    // adds the last chunk (if there is one)
    if !current.is_empty() {
        config_lines.push(current);
    }

    Ok(config_lines)
}

/// Send the `initialize_magic_hat` instruction to the magic hat program.
fn initialize_magic_hat(
    config_data: &ConfigData,
    magichat_account: &Keypair,
    magic_hat_data: MagicHatData,
    treasury_wallet: Pubkey,
    program: Program,
) -> Result<Signature> {
    let payer = program.payer();
    let items_available = magic_hat_data.items_available;

    let magichat_account_size = if magic_hat_data.hidden_settings.is_some() {
        CONFIG_ARRAY_START
    } else {
        CONFIG_ARRAY_START
            + 4
            + items_available as usize * CONFIG_LINE_SIZE
            + 8
            + 2 * (items_available as usize / 8 + 1)
    };

    info!(
        "Initializing Magic Hat with account size of: {} and address of: {}",
        magichat_account_size,
        magichat_account.pubkey().to_string()
    );

    let lamports = program
        .rpc()
        .get_minimum_balance_for_rent_exemption(magichat_account_size)?;

    let balance = program.rpc().get_account(&payer)?.lamports;

    if lamports > balance {
        return Err(DeployError::BalanceTooLow(
            format!("{:.3}", (balance as f64 / LAMPORTS_PER_SOL as f64)),
            format!("{:.3}", (lamports as f64 / LAMPORTS_PER_SOL as f64)),
        )
        .into());
    }

    let mut tx = program
        .request()
        .instruction(system_instruction::create_account(
            &payer,
            &magichat_account.pubkey(),
            lamports,
            magichat_account_size as u64,
            &program.id(),
        ))
        .signer(magichat_account)
        .accounts(nft_accounts::InitializeMagicHat {
            magic_hat: magichat_account.pubkey(),
            wallet: treasury_wallet,
            authority: payer,
            payer,
            system_program: system_program::id(),
            rent: sysvar::rent::ID,
        })
        .args(nft_instruction::InitializeMagicHat {
            data: magic_hat_data,
        });

    if let Some(token) = config_data.spl_token {
        tx = tx.accounts(AccountMeta {
            pubkey: token,
            is_signer: false,
            is_writable: false,
        });
    }

    let sig = tx.send()?;

    Ok(sig)
}

/// Send the config lines to the magic hat program.
async fn upload_config_lines(
    laddu_config: Arc<LadduConfig>,
    magichat_pubkey: Pubkey,
    cache: &mut Cache,
    config_lines: Vec<Vec<(u32, ConfigLine)>>,
    interrupted: Arc<AtomicBool>,
) -> Result<Vec<DeployError>> {
    println!(
        "Sending config line(s) in {} transaction(s): (Ctrl+C to abort)",
        config_lines.len()
    );

    let pb = progress_bar_with_style(config_lines.len() as u64);

    debug!("Num of config line chunks: {:?}", config_lines.len());
    info!("Uploading config lines in chunks...");

    let mut transactions = Vec::new();

    for chunk in config_lines {
        let keypair = bs58::encode(laddu_config.keypair.to_bytes()).into_string();
        let payer = Keypair::from_base58_string(&keypair);

        transactions.push(TxInfo {
            magichat_pubkey,
            payer,
            chunk,
        });
    }

    let mut handles = Vec::new();

    for tx in transactions.drain(0..cmp::min(transactions.len(), PARALLEL_LIMIT)) {
        let config = laddu_config.clone();
        handles.push(tokio::spawn(
            async move { add_config_lines(config, tx).await },
        ));
    }

    let mut errors = Vec::new();

    while !interrupted.load(Ordering::SeqCst) && !handles.is_empty() {
        match select_all(handles).await {
            (Ok(res), _index, remaining) => {
                // independently if the upload was successful or not
                // we continue to try the remaining ones
                handles = remaining;

                if res.is_ok() {
                    let indices = res?;

                    for index in indices {
                        let item = cache.items.0.get_mut(&index.to_string()).unwrap();
                        item.on_chain = true;
                    }
                    // updates the progress bar
                    pb.inc(1);
                } else {
                    // user will need to retry the upload
                    errors.push(DeployError::AddConfigLineFailed(format!(
                        "Transaction error: {:?}",
                        res.err().unwrap()
                    )));
                }
            }
            (Err(err), _index, remaining) => {
                // user will need to retry the upload
                errors.push(DeployError::AddConfigLineFailed(format!(
                    "Transaction error: {:?}",
                    err
                )));
                // ignoring all errors
                handles = remaining;
            }
        }

        if !transactions.is_empty() {
            // if we are half way through, let spawn more transactions
            if (PARALLEL_LIMIT - handles.len()) > (PARALLEL_LIMIT / 2) {
                // saves the progress to the cache file
                cache.sync_file()?;

                for tx in transactions.drain(0..cmp::min(transactions.len(), PARALLEL_LIMIT / 2)) {
                    let config = laddu_config.clone();
                    handles.push(tokio::spawn(
                        async move { add_config_lines(config, tx).await },
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        pb.abandon_with_message(format!("{}", style("Deploy failed ").red().bold()));
    } else if !transactions.is_empty() {
        pb.abandon_with_message(format!("{}", style("Upload aborted ").red().bold()));
        return Err(DeployError::AddConfigLineFailed(
            "Not all config lines were deployed.".to_string(),
        )
        .into());
    } else {
        pb.finish_with_message(format!("{}", style("Deploy successful ").green().bold()));
    }

    // makes sure the cache file is updated
    cache.sync_file()?;

    Ok(errors)
}

/// Send the `add_config_lines` instruction to the magic hat program.
async fn add_config_lines(config: Arc<LadduConfig>, tx_info: TxInfo) -> Result<Vec<u32>> {
    let client = setup_client(&config)?;
    let program = client.program(MAGIC_HAT_ID);

    // this will be used to update the cache
    let mut indices: Vec<u32> = Vec::new();
    // configLine does not implement clone, so we have to do this
    let mut config_lines: Vec<ConfigLine> = Vec::new();
    // start index
    let start_index = tx_info.chunk[0].0;

    for (index, line) in tx_info.chunk {
        indices.push(index);
        config_lines.push(line);
    }

    let _sig = program
        .request()
        .accounts(nft_accounts::AddConfigLines {
            magic_hat: tx_info.magichat_pubkey,
            authority: program.payer(),
        })
        .args(nft_instruction::AddConfigLines {
            index: start_index,
            config_lines,
        })
        .signer(&tx_info.payer)
        .send()?;

    Ok(indices)
}
