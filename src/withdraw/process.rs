pub use anchor_client::{
    solana_sdk::{
        commitment_config::{CommitmentConfig, CommitmentLevel},
        native_token::LAMPORTS_PER_SOL,
        pubkey::Pubkey,
        signature::{Keypair, Signature, Signer},
        system_instruction, system_program, sysvar,
        transaction::Transaction,
    },
    Client, Program,
};
use console::style;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use std::{
    io::{stdin, stdout, Write},
    rc::Rc,
    str::FromStr,
};

use magic_hat::accounts as nft_accounts;
use magic_hat::instruction as nft_instruction;

use crate::common::*;
use crate::magic_hat::MAGIC_HAT_ID;
use crate::setup::{laddu_setup, setup_client};
use crate::utils::*;

pub struct WithdrawArgs {
    pub magic_hat: Option<String>,
    pub keypair: Option<String>,
    pub rpc_url: Option<String>,
    pub list: bool,
}

pub fn process_withdraw(args: WithdrawArgs) -> Result<()> {
    // (1) Setting up connection

    println!(
        "{} {}Initializing connection",
        style("[1/2]").bold().dim(),
        COMPUTER_EMOJI
    );

    let pb = spinner_with_style();
    pb.set_message("Connecting...");

    let (program, payer) = setup_withdraw(args.keypair, args.rpc_url)?;

    pb.finish_with_message("Connected");

    println!(
        "\n{} {}{} funds",
        style("[2/2]").bold().dim(),
        WITHDRAW_EMOJI,
        if args.list { "Listing" } else { "Retrieving" }
    );

    // the --list flag takes precedence; even if a magic hat id is passed
    // as an argument, we will list the magic hats (no draining happens)
    let magic_hat = if args.list { None } else { args.magic_hat };

    // (2) Retrieving data for listing/draining

    match &magic_hat {
        Some(magic_hat) => {
            let magic_hat = Pubkey::from_str(magic_hat)?;

            let pb = spinner_with_style();
            pb.set_message("Draining Magic Hat...");

            do_withdraw(Rc::new(program), magic_hat, payer)?;

            pb.finish_with_message("Done");
        }
        None => {
            let config = RpcProgramAccountsConfig {
                filters: Some(vec![RpcFilterType::Memcmp(Memcmp {
                    offset: 8, // key
                    bytes: MemcmpEncodedBytes::Base58(payer.to_string()),
                    encoding: None,
                })]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    data_slice: None,
                    commitment: Some(CommitmentConfig {
                        commitment: CommitmentLevel::Confirmed,
                    }),
                },
                with_context: None,
            };

            let pb = spinner_with_style();
            pb.set_message("Looking up Magic Hats...");

            let program = Rc::new(program);
            let accounts = program
                .rpc()
                .get_program_accounts_with_config(&program.id(), config)?;

            pb.finish_and_clear();

            let mut total = 0.0f64;

            accounts.iter().for_each(|account| {
                let (_pubkey, account) = account;
                total += account.lamports as f64;
            });

            println!(
                "Found {} Magic Hats, total amount: ◎ {}",
                accounts.len(),
                total / LAMPORTS_PER_SOL as f64
            );

            if accounts.is_empty() {
                // nothing else to do, we just say goodbye
                println!("\n{}", style("[Completed]").bold().dim());
            } else if args.list {
                println!("\n{:48} Balance", "Magic Hat ID");
                println!("{:-<61}", "-");

                for (pubkey, account) in accounts {
                    println!(
                        "{:48} {:>12.8}",
                        pubkey.to_string(),
                        account.lamports as f64 / LAMPORTS_PER_SOL as f64
                    );
                }

                println!("\n{}", style("[Completed]").bold().dim());
            } else {
                println!("\n+----------------------------------------------+");
                println!("| WARNING: This will drain all Magic Hats. |");
                println!("+----------------------------------------------+");

                print!("\nContinue? [Y/n] (default \'n\'): ");
                stdout().flush().ok();

                let mut s = String::new();
                stdin().read_line(&mut s).expect("Error reading input.");

                if let Some('Y') = s.chars().next() {
                    let pb = progress_bar_with_style(accounts.len() as u64);
                    let mut not_drained = 0;

                    accounts.iter().for_each(|account| {
                        let (magic_hat, _account) = account;
                        do_withdraw(program.clone(), *magic_hat, payer).unwrap_or_else(|e| {
                            not_drained += 1;
                            error!("Error: {}", e);
                        });
                        pb.inc(1);
                    });

                    pb.finish();

                    if not_drained > 0 {
                        println!(
                            "{}",
                            style(format!("Could not drain {} Magic Hat(s)", not_drained))
                                .red()
                                .bold()
                                .dim()
                        );
                    }
                } else {
                    // there were magic hats to drain, but the user decided
                    // to abort the withdraw
                    println!("\n{}", style("Withdraw aborted.").red().bold().dim());
                }
            }
        }
    }

    Ok(())
}

fn setup_withdraw(keypair: Option<String>, rpc_url: Option<String>) -> Result<(Program, Pubkey)> {
    let laddu_config = laddu_setup(keypair, rpc_url)?;
    let client = setup_client(&laddu_config)?;
    let program = client.program(MAGIC_HAT_ID);
    let payer = program.payer();

    Ok((program, payer))
}

fn do_withdraw(program: Rc<Program>, magic_hat: Pubkey, payer: Pubkey) -> Result<()> {
    program
        .request()
        .accounts(nft_accounts::WithdrawFunds {
            magic_hat,
            authority: payer,
        })
        .args(nft_instruction::WithdrawFunds {})
        .send()?;

    Ok(())
}
