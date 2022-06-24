use std::str::FromStr;

use anchor_client::solana_sdk::pubkey::Pubkey;
use anyhow::Result;
use console::style;
use magic_hat::accounts as nft_accounts;
use magic_hat::instruction as nft_instruction;
use mpl_token_metadata::pda::find_collection_authority_account;
use mpl_token_metadata::state::Metadata;

use crate::cache::load_cache;
use crate::common::*;
use crate::magic_hat::MAGIC_HAT_ID;
use crate::magic_hat::*;
use crate::pdas::*;
use crate::utils::spinner_with_style;

pub struct RemoveCollectionArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub magic_hat: Option<String>,
}

pub fn process_remove_collection(args: RemoveCollectionArgs) -> Result<()> {
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

    let magic_hat_state = get_magic_hat_state(&laddu_config, &magichat_pubkey)?;
    let (collection_pda_pubkey, collection_pda) = get_collection_pda(&magichat_pubkey, &program)?;
    let collection_mint_pubkey = collection_pda.mint;
    let collection_metadata_info = get_metadata_pda(&collection_mint_pubkey, &program)?;

    pb.finish_with_message("Done");

    println!(
        "{} {}Removing collection mint for magic hat",
        style("[2/2]").bold().dim(),
        MAGICHAT_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Sending remove collection transaction...");

    let remove_signature = remove_collection(
        &program,
        &magichat_pubkey,
        &magic_hat_state,
        &collection_pda_pubkey,
        &collection_mint_pubkey,
        &collection_metadata_info,
    )?;

    pb.finish_with_message(format!(
        "{} {}",
        style("Remove collection signature:").bold(),
        remove_signature
    ));

    Ok(())
}

pub fn remove_collection(
    program: &Program,
    magichat_pubkey: &Pubkey,
    magic_hat_state: &MagicHat,
    collection_pda_pubkey: &Pubkey,
    collection_mint_pubkey: &Pubkey,
    collection_metadata_info: &PdaInfo<Metadata>,
) -> Result<Signature> {
    let payer = program.payer();

    let collection_authority_record =
        find_collection_authority_account(collection_mint_pubkey, collection_pda_pubkey).0;

    let (collection_metadata_pubkey, collection_metadata) = collection_metadata_info;

    if collection_metadata.update_authority != payer {
        return Err(anyhow!(CustomMagicHatError::AuthorityMismatch(
            collection_metadata.update_authority.to_string(),
            payer.to_string()
        )));
    }

    if magic_hat_state.items_redeemed > 0 {
        return Err(anyhow!(
            "You can't modify the Magic Hat collection after items have been minted."
        ));
    }

    let builder = program
        .request()
        .accounts(nft_accounts::RemoveCollection {
            magic_hat: *magichat_pubkey,
            authority: payer,
            collection_pda: *collection_pda_pubkey,
            metadata: *collection_metadata_pubkey,
            mint: *collection_mint_pubkey,
            collection_authority_record,
            token_metadata_program: mpl_token_metadata::ID,
        })
        .args(nft_instruction::RemoveCollection);

    let sig = builder.send()?;

    Ok(sig)
}
