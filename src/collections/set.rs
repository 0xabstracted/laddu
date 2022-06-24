use std::str::FromStr;

use anchor_client::solana_sdk::{pubkey::Pubkey, system_program, sysvar};
use anyhow::Result;
use console::style;
use magic_hat::instruction as nft_instruction;
use magic_hat::{accounts as nft_accounts, MagicHatError};
use mpl_token_metadata::error::MetadataError;
use mpl_token_metadata::pda::find_collection_authority_account;
use mpl_token_metadata::state::{MasterEditionV2, Metadata};

use crate::cache::load_cache;
use crate::common::*;
use crate::magic_hat::MAGIC_HAT_ID;
use crate::magic_hat::*;
use crate::pdas::*;
use crate::utils::spinner_with_style;

pub struct SetCollectionArgs {
    pub collection_mint: String,
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub magic_hat: Option<String>,
}

pub fn process_set_collection(args: SetCollectionArgs) -> Result<()> {
    let laddu_config = laddu_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&laddu_config)?;
    let program = client.program(MAGIC_HAT_ID);

    // the magic hat id specified takes precedence over the one from the cache
    let magic_hat_id = match args.magic_hat {
        Some(magic_hat_id) => magic_hat_id,
        None => {
            let cache = load_cache(&args.cache, false)?;
            cache.program.magic_hat
        }
    };

    let collection_mint_pubkey = match Pubkey::from_str(&args.collection_mint) {
        Ok(magichat_pubkey) => magichat_pubkey,
        Err(_) => {
            let error = anyhow!(
                "Failed to parse collection mint id: {}",
                args.collection_mint
            );
            error!("{:?}", error);
            return Err(error);
        }
    };

    let magichat_pubkey = match Pubkey::from_str(&magic_hat_id) {
        Ok(magichat_pubkey) => magichat_pubkey,
        Err(_) => {
            let error = anyhow!("Failed to parse Magic Hat {}", magic_hat_id);
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

    let magic_hat_state = get_magic_hat_state(&laddu_config, &Pubkey::from_str(&magic_hat_id)?)?;

    let collection_metadata_info = get_metadata_pda(&collection_mint_pubkey, &program)?;

    let collection_edition_info = get_master_edition_pda(&collection_mint_pubkey, &program)?;

    pb.finish_with_message("Done");

    println!(
        "{} {}Setting collection mint for Magic Hat",
        style("[2/2]").bold().dim(),
        MAGICHAT_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Sending set collection transaction...");

    let set_signature = set_collection(
        &program,
        &magichat_pubkey,
        &magic_hat_state,
        &collection_mint_pubkey,
        &collection_metadata_info,
        &collection_edition_info,
    )?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Set collection signature:").bold(),
        set_signature
    ));

    Ok(())
}

pub fn set_collection(
    program: &Program,
    magichat_pubkey: &Pubkey,
    magic_hat_state: &MagicHat,
    collection_mint_pubkey: &Pubkey,
    collection_metadata_info: &PdaInfo<Metadata>,
    collection_edition_info: &PdaInfo<MasterEditionV2>,
) -> Result<Signature> {
    let payer = program.payer();

    let collection_pda_pubkey = find_collection_pda(magichat_pubkey).0;
    let (collection_metadata_pubkey, collection_metadata) = collection_metadata_info;
    let (collection_edition_pubkey, collection_edition) = collection_edition_info;

    let collection_authority_record =
        find_collection_authority_account(collection_mint_pubkey, &collection_pda_pubkey).0;

    if !magic_hat_state.data.retain_authority {
        return Err(anyhow!(
            MagicHatError::MagicHatCollectionRequiresRetainAuthority
        ));
    }

    if collection_metadata.update_authority != payer {
        return Err(anyhow!(CustomMagicHatError::AuthorityMismatch(
            collection_metadata.update_authority.to_string(),
            payer.to_string()
        )));
    }

    if collection_edition.max_supply != Some(0) {
        return Err(anyhow!(MetadataError::CollectionMustBeAUniqueMasterEdition));
    }

    if magic_hat_state.items_redeemed > 0 {
        return Err(anyhow!(
            "You can't modify the Magic Hat collection after items have been minted."
        ));
    }

    let builder = program
        .request()
        .accounts(nft_accounts::SetCollection {
            magic_hat: *magichat_pubkey,
            authority: payer,
            collection_pda: collection_pda_pubkey,
            payer,
            system_program: system_program::id(),
            rent: sysvar::rent::ID,
            metadata: *collection_metadata_pubkey,
            mint: *collection_mint_pubkey,
            edition: *collection_edition_pubkey,
            collection_authority_record,
            token_metadata_program: mpl_token_metadata::ID,
        })
        .args(nft_instruction::SetCollection);

    let sig = builder.send()?;

    Ok(sig)
}
