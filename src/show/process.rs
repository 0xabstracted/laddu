use std::str::FromStr;

use anchor_client::solana_sdk::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey};
use anyhow::Result;
use chrono::NaiveDateTime;
use console::style;
use magic_hat::{EndSettingType, WhitelistMintMode};

use crate::cache::load_cache;
use crate::common::*;
use crate::magic_hat::*;
use crate::pdas::get_collection_pda;
use crate::utils::*;

pub struct ShowArgs {
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub cache: String,
    pub magic_hat: Option<String>,
}

pub fn process_show(args: ShowArgs) -> Result<()> {
    println!(
        "{} {}Looking up Magic Hat",
        style("[1/1]").bold().dim(),
        LOOKING_GLASS_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    // the magic hat id specified takes precedence over the one from the cache

    let magic_hat_id = if let Some(magic_hat) = args.magic_hat {
        magic_hat
    } else {
        let cache = load_cache(&args.cache, false)?;
        cache.program.magic_hat
    };

    let laddu_config = laddu_setup(args.keypair, args.rpc_url)?;
    let client = setup_client(&laddu_config)?;
    let program = client.program(MAGIC_HAT_ID);

    let magic_hat_id = match Pubkey::from_str(&magic_hat_id) {
        Ok(magic_hat_id) => magic_hat_id,
        Err(_) => {
            let error = anyhow!("Failed to parse Magic Hat id: {}", magic_hat_id);
            error!("{:?}", error);
            return Err(error);
        }
    };

    let collection_mint =
        if let Ok((_, collection_pda)) = get_collection_pda(&magic_hat_id, &program) {
            Some(collection_pda.mint)
        } else {
            None
        };

    let cndy_state = get_magic_hat_state(&laddu_config, &magic_hat_id)?;
    let cndy_data = cndy_state.data;

    pb.finish_and_clear();

    println!(
        "\n{}{} {}",
        MAGICHAT_EMOJI,
        style("Magic Hat ID:").dim(),
        &magic_hat_id
    );

    // magic hat state and data

    println!(" {}", style(":").dim());
    print_with_style("", "authority", cndy_state.authority.to_string());
    print_with_style("", "wallet", cndy_state.wallet.to_string());
    match collection_mint {
        Some(collection_mint) => {
            print_with_style("", "collection mint", collection_mint.to_string())
        }
        None => print_with_style("", "collection mint", "none".to_string()),
    };

    if let Some(token_mint) = cndy_state.token_mint {
        print_with_style("", "spl token", token_mint.to_string());
    } else {
        print_with_style("", "spl token", "none".to_string());
    }

    print_with_style("", "max supply", cndy_data.max_supply.to_string());
    print_with_style("", "items redeemed", cndy_state.items_redeemed.to_string());
    print_with_style("", "items available", cndy_data.items_available.to_string());

    print_with_style("", "uuid", cndy_data.uuid.to_string());
    print_with_style(
        "",
        "price",
        format!(
            "◎ {} ({})",
            cndy_data.price as f64 / LAMPORTS_PER_SOL as f64,
            cndy_data.price
        ),
    );
    print_with_style("", "symbol", cndy_data.symbol.to_string());
    print_with_style(
        "",
        "seller fee basis points",
        format!(
            "{}% ({})",
            cndy_data.seller_fee_basis_points / 100,
            cndy_data.seller_fee_basis_points
        ),
    );
    print_with_style("", "is mutable", cndy_data.is_mutable.to_string());
    print_with_style(
        "",
        "retain authority",
        cndy_data.retain_authority.to_string(),
    );
    if let Some(date) = cndy_data.go_live_date {
        let date = NaiveDateTime::from_timestamp(date, 0);
        print_with_style(
            "",
            "go live date",
            date.format("%a %B %e %Y %H:%M:%S UTC").to_string(),
        );
    } else {
        print_with_style("", "go live date", "none".to_string());
    }
    print_with_style("", "creators", "".to_string());

    for (index, creator) in cndy_data.creators.into_iter().enumerate() {
        let info = format!(
            "{} ({}%{})",
            creator.address,
            creator.share,
            if creator.verified { ", verified" } else { "" },
        );
        print_with_style(":   ", &(index + 1).to_string(), info);
    }

    // end settings
    if let Some(end_settings) = cndy_data.end_settings {
        print_with_style("", "end settings", "".to_string());
        match end_settings.end_setting_type {
            EndSettingType::Date => {
                print_with_style(":   ", "end setting type", "date".to_string());
                let date = NaiveDateTime::from_timestamp(end_settings.number as i64, 0);
                print_with_style(
                    ":   ",
                    "number",
                    date.format("%a %B %e %Y %H:%M:%S UTC").to_string(),
                );
            }
            EndSettingType::Amount => {
                print_with_style(":   ", "end setting type", "amount".to_string());
                print_with_style(":   ", "number", end_settings.number.to_string());
            }
        }
    } else {
        print_with_style("", "end settings", "none".to_string());
    }

    // hidden settings
    if let Some(hidden_settings) = cndy_data.hidden_settings {
        print_with_style("", "hidden settings", "".to_string());
        print_with_style(":   ", "name", hidden_settings.name);
        print_with_style(":   ", "uri", hidden_settings.uri);
        print_with_style(
            ":   ",
            "hash",
            String::from_utf8(hidden_settings.hash.to_vec())?,
        );
    } else {
        print_with_style("", "hidden settings", "none".to_string());
    }

    // whitelist mint settings
    if let Some(whitelist_settings) = cndy_data.whitelist_mint_settings {
        print_with_style("", "whitelist mint settings", "".to_string());
        print_with_style(
            ":   ",
            "mode",
            if whitelist_settings.mode == WhitelistMintMode::BurnEveryTime {
                "burn every time".to_string()
            } else {
                "never burn".to_string()
            },
        );
        print_with_style(":   ", "mint", whitelist_settings.mint.to_string());
        print_with_style(":   ", "presale", whitelist_settings.presale.to_string());
        print_with_style(
            ":   ",
            "discount price",
            if let Some(value) = whitelist_settings.discount_price {
                format!("◎ {} ({})", value as f64 / LAMPORTS_PER_SOL as f64, value)
            } else {
                "none".to_string()
            },
        );
    } else {
        print_with_style("", "whitelist mint settings", "none".to_string());
    }

    // gatekeeper settings
    if let Some(gatekeeper) = cndy_data.gatekeeper {
        print_with_style("", "gatekeeper", "".to_string());
        print_with_style(
            "    ",
            "gatekeeper network",
            gatekeeper.gatekeeper_network.to_string(),
        );
        print_with_style(
            "    ",
            "expire on use",
            gatekeeper.expire_on_use.to_string(),
        );
    } else {
        print_with_style("", "gatekeeper", "none".to_string());
    }

    Ok(())
}

fn print_with_style(indent: &str, key: &str, value: String) {
    println!(
        " {} {}",
        style(format!("{}:.. {}:", indent, key)).dim(),
        value
    );
}
