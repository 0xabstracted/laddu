use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::{Client, ClientError};
use anyhow::{anyhow, Result};
pub use magic_hat::ID as MAGIC_HAT_ID;
use magic_hat::{MagicHat, MagicHatData, WhitelistMintMode, WhitelistMintSettings};
use spl_token::id as token_program_id;

use crate::config::data::LadduConfig;
use crate::config::{price_as_lamports, ConfigData};
use crate::setup::setup_client;
use crate::utils::check_spl_token;

// To test a custom magichat program, comment the line above and use the
// following lines to declare the id to use:
//use solana_program::declare_id;
//declare_id!("<Magic Hat ID>");

#[derive(Debug)]
pub struct ConfigStatus {
    pub index: u32,
    pub on_chain: bool,
}

pub fn parse_config_price(client: &Client, config: &ConfigData) -> Result<u64> {
    let parsed_price = if let Some(spl_token) = config.spl_token {
        let token_program = client.program(token_program_id());
        let token_mint = check_spl_token(&token_program, &spl_token.to_string())?;

        match (config.price as u64).checked_mul(10u64.pow(token_mint.decimals.into())) {
            Some(price) => price,
            None => return Err(anyhow!("Price math overflow")),
        }
    } else {
        price_as_lamports(config.price)
    };

    Ok(parsed_price)
}

pub fn get_magic_hat_state(laddu_config: &LadduConfig, magic_hat_id: &Pubkey) -> Result<MagicHat> {
    let client = setup_client(laddu_config)?;
    let program = client.program(MAGIC_HAT_ID);

    program.account(*magic_hat_id).map_err(|e| match e {
        ClientError::AccountNotFound => anyhow!("Magic Hat does not exist!"),
        _ => anyhow!(
            "Failed to deserialize Magic Hat account: {}",
            magic_hat_id.to_string()
        ),
    })
}

pub fn get_magic_hat_data(
    laddu_config: &LadduConfig,
    magic_hat_id: &Pubkey,
) -> Result<MagicHatData> {
    let magic_hat = get_magic_hat_state(laddu_config, magic_hat_id)?;
    Ok(magic_hat.data)
}

pub fn print_magic_hat_state(state: MagicHat) {
    println!("Authority {:?}", state.authority);
    println!("Wallet {:?}", state.wallet);
    println!("Token mint: {:?}", state.token_mint);
    println!("Items redeemed: {:?}", state.items_redeemed);
    print_magic_hat_data(&state.data);
}

pub fn print_magic_hat_data(data: &MagicHatData) {
    println!("Uuid: {:?}", data.uuid);
    println!("Price: {:?}", data.price);
    println!("Symbol: {:?}", data.symbol);
    println!(
        "Seller fee basis points: {:?}",
        data.seller_fee_basis_points
    );
    println!("Max supply: {:?}", data.max_supply);
    println!("Is mutable: {:?}", data.is_mutable);
    println!("Retain Authority: {:?}", data.retain_authority);
    println!("Go live date: {:?}", data.go_live_date);
    println!("Items available: {:?}", data.items_available);

    print_whitelist_mint_settings(&data.whitelist_mint_settings);
}

fn print_whitelist_mint_settings(settings: &Option<WhitelistMintSettings>) {
    if let Some(settings) = settings {
        match settings.mode {
            WhitelistMintMode::BurnEveryTime => println!("Mode: Burn every time"),
            WhitelistMintMode::NeverBurn => println!("Mode: Never burn"),
        }
        println!("Mint: {:?}", settings.mint);
        println!("Presale: {:?}", settings.presale);
        println!("Discount price: {:?}", settings.discount_price);
    } else {
        println!("No whitelist mint settings");
    }
}
