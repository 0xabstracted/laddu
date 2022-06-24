use std::{str::FromStr, sync::Arc};

use anchor_client::{
    solana_sdk::{
        program_pack::Pack,
        pubkey::Pubkey,
        signature::{Keypair, Signature, Signer},
        system_instruction, system_program, sysvar,
    },
    Client,
};
use anchor_lang::prelude::AccountMeta;
use anyhow::Result;
use chrono::Utc;
use console::style;
use magic_hat::instruction as nft_instruction;
use magic_hat::{accounts as nft_accounts, CollectionPDA};
use magic_hat::{EndSettingType, MagicHat, MagicHatError, WhitelistMintMode};
use mpl_token_metadata::pda::find_collection_authority_account;
use solana_client::rpc_response::Response;
use spl_associated_token_account::{create_associated_token_account, get_associated_token_address};
use spl_token::{
    instruction::{initialize_mint, mint_to},
    state::Account,
    ID as TOKEN_PROGRAM_ID,
};

use crate::cache::load_cache;
use crate::common::*;
use crate::config::Cluster;
use crate::magic_hat::MAGIC_HAT_ID;
use crate::magic_hat::*;
use crate::pdas::*;
use crate::utils::*;

pub struct MintArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub number: Option<u64>,
    pub magic_hat: Option<String>,
}

pub fn process_mint(args: MintArgs) -> Result<()> {
    let laddu_config = laddu_setup(args.keypair, args.rpc_url)?;
    let client = Arc::new(setup_client(&laddu_config)?);

    // the magic hat id specified takes precedence over the one from the cache

    let magic_hat_id = match args.magic_hat {
        Some(magic_hat_id) => magic_hat_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.magic_hat
        }
    };

    let magichat_pubkey = match Pubkey::from_str(&magic_hat_id) {
        Ok(magichat_pubkey) => magichat_pubkey,
        Err(_) => {
            let error = anyhow!("Failed to parse Magic Hat id: {}", magic_hat_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    println!(
        "{} {}Loading Magic Hat",
        style("[1/2]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );
    println!("{} {}", style("Magic Hat ID:").bold(), magic_hat_id);

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let magic_hat_state = Arc::new(get_magic_hat_state(&laddu_config, &magichat_pubkey)?);

    let collection_pda_info =
        Arc::new(get_collection_pda(&magichat_pubkey, &client.program(MAGIC_HAT_ID)).ok());

    pb.finish_with_message("Done");

    println!(
        "{} {}Minting from Magic Hat",
        style("[2/2]").bold().dim(),
        MAGICHAT_EMOJI
    );
    println!("Magic Hat ID: {}", &magic_hat_id);

    let number = args.number.unwrap_or(1);
    let available = magic_hat_state.data.items_available - magic_hat_state.items_redeemed;

    if number > available || number == 0 {
        let error = anyhow!("{} item(s) available, requested {}", available, number);
        error!("{:?}", error);
        return Err(error);
    }

    info!("Minting NFT from Magic Hat: {}", &magic_hat_id);
    info!("Magic Hat program id: {:?}", MAGIC_HAT_ID);

    if number == 1 {
        let pb = spinner_with_style();
        pb.set_message(format!(
            "{} item(s) remaining",
            magic_hat_state.data.items_available - magic_hat_state.items_redeemed
        ));

        let result = match mint(
            Arc::clone(&client),
            magichat_pubkey,
            Arc::clone(&magic_hat_state),
            Arc::clone(&collection_pda_info),
        ) {
            Ok(signature) => format!("{} {}", style("Signature:").bold(), signature),
            Err(err) => {
                pb.abandon_with_message(format!("{}", style("Mint failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }
        };

        pb.finish_with_message(result);
    } else {
        let pb = progress_bar_with_style(number);

        for _i in 0..number {
            if let Err(err) = mint(
                Arc::clone(&client),
                magichat_pubkey,
                Arc::clone(&magic_hat_state),
                Arc::clone(&collection_pda_info),
            ) {
                pb.abandon_with_message(format!("{}", style("Mint failed ").red().bold()));
                error!("{:?}", err);
                return Err(err);
            }

            pb.inc(1);
        }

        pb.finish();
    }

    Ok(())
}

pub fn mint(
    client: Arc<Client>,
    magic_hat_id: Pubkey,
    magic_hat_state: Arc<MagicHat>,
    collection_pda_info: Arc<Option<PdaInfo<CollectionPDA>>>,
) -> Result<Signature> {
    let program = client.program(MAGIC_HAT_ID);
    let payer = program.payer();
    let wallet = magic_hat_state.wallet;

    let magic_hat_data = &magic_hat_state.data;

    if let Some(_gatekeeper) = &magic_hat_data.gatekeeper {
        return Err(anyhow!(
            "Command-line mint disabled (gatekeeper settings in use)"
        ));
    } else if magic_hat_state.items_redeemed >= magic_hat_data.items_available {
        return Err(anyhow!(MagicHatError::MagicHatEmpty));
    }

    if magic_hat_state.authority != payer {
        // we are not authority, we need to follow the rules
        // 1. go_live_date
        // 2. whitelist mint settings
        // 3. end settings
        let mint_date = Utc::now().timestamp();
        let mut mint_enabled = if let Some(date) = magic_hat_data.go_live_date {
            // mint will be enabled only if the go live date is earlier
            // than the current date
            date < mint_date
        } else {
            // this is the case that go live date is null
            false
        };

        if let Some(wl_mint_settings) = &magic_hat_data.whitelist_mint_settings {
            if wl_mint_settings.presale {
                // we (temporarily) enable the mint - we will validate if the user
                // has the wl token when creating the transaction
                mint_enabled = true;
            } else if !mint_enabled {
                return Err(anyhow!(MagicHatError::MagicHatNotLive));
            }
        }

        if !mint_enabled {
            // no whitelist mint settings (or no presale) and we are earlier than
            // go live date
            return Err(anyhow!(MagicHatError::MagicHatNotLive));
        }

        if let Some(end_settings) = &magic_hat_data.end_settings {
            match end_settings.end_setting_type {
                EndSettingType::Date => {
                    if (end_settings.number as i64) < mint_date {
                        return Err(anyhow!(MagicHatError::MagicHatNotLive));
                    }
                }
                EndSettingType::Amount => {
                    if magic_hat_state.items_redeemed >= end_settings.number {
                        return Err(anyhow!(
                            "Magic Hat is not live (end settings amount reached)"
                        ));
                    }
                }
            }
        }
    }

    let nft_mint = Keypair::new();
    let metaplex_program_id = Pubkey::from_str(METAPLEX_PROGRAM_ID)?;

    // Allocate memory for the account
    let min_rent = program
        .rpc()
        .get_minimum_balance_for_rent_exemption(MINT_LAYOUT as usize)?;

    // Create mint account
    let create_mint_account_ix = system_instruction::create_account(
        &payer,
        &nft_mint.pubkey(),
        min_rent,
        MINT_LAYOUT,
        &TOKEN_PROGRAM_ID,
    );

    // Initialize mint ix
    let init_mint_ix = initialize_mint(
        &TOKEN_PROGRAM_ID,
        &nft_mint.pubkey(),
        &payer,
        Some(&payer),
        0,
    )?;

    // Derive associated token account
    let assoc = get_associated_token_address(&payer, &nft_mint.pubkey());

    // Create associated account instruction
    let create_assoc_account_ix =
        create_associated_token_account(&payer, &payer, &nft_mint.pubkey());

    // Mint to instruction
    let mint_to_ix = mint_to(
        &TOKEN_PROGRAM_ID,
        &nft_mint.pubkey(),
        &assoc,
        &payer,
        &[],
        1,
    )?;

    let mut additional_accounts: Vec<AccountMeta> = Vec::new();

    // Check whitelist mint settings
    if let Some(wl_mint_settings) = &magic_hat_data.whitelist_mint_settings {
        let whitelist_token_account = get_associated_token_address(&payer, &wl_mint_settings.mint);

        additional_accounts.push(AccountMeta {
            pubkey: whitelist_token_account,
            is_signer: false,
            is_writable: true,
        });

        if wl_mint_settings.mode == WhitelistMintMode::BurnEveryTime {
            let mut token_found = false;

            match program.rpc().get_account_data(&whitelist_token_account) {
                Ok(ata_data) => {
                    if !ata_data.is_empty() {
                        let account = Account::unpack_unchecked(&ata_data)?;

                        if account.amount > 0 {
                            additional_accounts.push(AccountMeta {
                                pubkey: wl_mint_settings.mint,
                                is_signer: false,
                                is_writable: true,
                            });

                            additional_accounts.push(AccountMeta {
                                pubkey: payer,
                                is_signer: true,
                                is_writable: false,
                            });

                            token_found = true;
                        }
                    }
                }
                Err(err) => return Err(anyhow!(err)),
            }

            if !token_found {
                return Err(anyhow!(MagicHatError::NoWhitelistToken));
            }
        }
    }

    if let Some(token_mint) = magic_hat_state.token_mint {
        let user_token_account_info = get_associated_token_address(&payer, &token_mint);

        additional_accounts.push(AccountMeta {
            pubkey: user_token_account_info,
            is_signer: false,
            is_writable: true,
        });

        additional_accounts.push(AccountMeta {
            pubkey: payer,
            is_signer: true,
            is_writable: false,
        })
    }

    let metadata_pda = find_metadata_pda(&nft_mint.pubkey());
    let master_edition_pda = find_master_edition_pda(&nft_mint.pubkey());
    let (magic_hat_creator_pda, creator_bump) = find_magic_hat_creator_pda(&magic_hat_id);

    let mint_ix = program
        .request()
        .accounts(nft_accounts::MintNFT {
            magic_hat: magic_hat_id,
            magic_hat_creator: magic_hat_creator_pda,
            payer,
            wallet,
            metadata: metadata_pda,
            mint: nft_mint.pubkey(),
            mint_authority: payer,
            update_authority: payer,
            master_edition: master_edition_pda,
            token_metadata_program: metaplex_program_id,
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program::id(),
            rent: sysvar::rent::ID,
            clock: sysvar::clock::ID,
            recent_blockhashes: sysvar::recent_blockhashes::ID,
            instruction_sysvar_account: sysvar::instructions::ID,
        })
        .args(nft_instruction::MintNft { creator_bump })
        .instructions()?;

    let mut builder = program
        .request()
        .instruction(create_mint_account_ix)
        .instruction(init_mint_ix)
        .instruction(create_assoc_account_ix)
        .instruction(mint_to_ix)
        .signer(&nft_mint)
        .instruction(mint_ix[0].clone());

    if !additional_accounts.is_empty() {
        for account in additional_accounts {
            builder = builder.accounts(account);
        }
    }

    if let Some((collection_pda_pubkey, collection_pda)) = collection_pda_info.as_ref() {
        let collection_authority_record =
            find_collection_authority_account(&collection_pda.mint, collection_pda_pubkey).0;
        builder = builder
            .accounts(nft_accounts::SetCollectionDuringMint {
                magic_hat: magic_hat_id,
                metadata: metadata_pda,
                payer,
                collection_pda: *collection_pda_pubkey,
                token_metadata_program: mpl_token_metadata::ID,
                instructions: sysvar::instructions::ID,
                collection_mint: collection_pda.mint,
                collection_metadata: find_metadata_pda(&collection_pda.mint),
                collection_master_edition: find_master_edition_pda(&collection_pda.mint),
                authority: payer,
                collection_authority_record,
            })
            .args(nft_instruction::SetCollectionDuringMint {});
    }

    let sig = builder.send()?;

    if let Err(_) | Ok(Response { value: None, .. }) = program
        .rpc()
        .get_account_with_commitment(&metadata_pda, CommitmentConfig::processed())
    {
        let cluster_param = match get_cluster(program.rpc()).unwrap_or(Cluster::Mainnet) {
            Cluster::Devnet => "?devnet",
            Cluster::Mainnet => "",
        };
        return Err(anyhow!(
            "Minting most likely failed with a bot tax. Check the transaction link for more details: https://explorer.solana.com/tx/{}{}",
            sig.to_string(),
            cluster_param,
        ));
    }

    info!("Minted! TxId: {}", sig);

    Ok(sig)
}
