use anyhow::Context;
use anyhow::{anyhow, Error};

use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

use substreams_solana_utils::spl_token::TOKEN_PROGRAM_ID;
use substreams_solana_utils as utils;
use utils::instruction::{get_structured_instructions, StructuredInstruction, StructuredInstructions};
use utils::system_program::SYSTEM_PROGRAM_ID;
use utils::transaction::{get_context, TransactionContext};
use utils::log::Log;

pub mod pumpfun;
use pumpfun::PUMPFUN_PROGRAM_ID;
use pumpfun::log::PumpfunLog;
use pumpfun::instruction::PumpfunInstruction;

pub mod pb;
use pb::pumpfun::*;
use pb::pumpfun::pumpfun_event::Event;

use system_program_substream;

fn pumpfun_events(block: Block) -> Result<PumpfunBlockEvents, Error> {
    let transactions = parse_block(&block)?;
    Ok(PumpfunBlockEvents { transactions })
}

pub fn parse_block(block: &Block) -> Result<Vec<PumpfunTransactionEvents>, Error> {
    let mut block_events: Vec<PumpfunTransactionEvents> = Vec::new();
    for transaction in block.transactions() {
        let events = parse_transaction(transaction)?;
        if !events.is_empty() {
            block_events.push(PumpfunTransactionEvents {
                signature: utils::transaction::get_signature(&transaction),
                events,
            });
        }
    }
    Ok(block_events)
}

pub fn parse_transaction(transaction: &ConfirmedTransaction) -> Result<Vec<PumpfunEvent>, Error> {
    if let Some(_) = transaction.meta.as_ref().unwrap().err {
        return Ok(Vec::new())
    }

    let mut events: Vec<PumpfunEvent> = Vec::new();

    let context = get_context(transaction).unwrap();
    let instructions = get_structured_instructions(transaction).unwrap();

    for instruction in instructions.flattened().iter() {
        if instruction.program_id() != PUMPFUN_PROGRAM_ID {
            continue;
        }

        match parse_instruction(&instruction, &context) {
            Ok(Some(event)) => {
                events.push(PumpfunEvent {
                    event: Some(event),
                })
            }
            Ok(None) => (),
            Err(error) => return Err(anyhow!("Transaction {} error: {}", &context.signature, error)),
        }
    }
    Ok(events)
}

pub fn parse_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext
) -> Result<Option<Event>, Error> {
    if instruction.program_id() != PUMPFUN_PROGRAM_ID {
        return Err(anyhow!("Not a Pumpfun instruction."));
    }
    let unpacked = PumpfunInstruction::unpack(instruction.data()).map_err(|x| anyhow!(x))?;
    match unpacked {
        PumpfunInstruction::Initialize => {
            Ok(Some(Event::Initialize(_parse_initialize_instruction(instruction, context)?)))
        },
        PumpfunInstruction::SetParams(set_params) => {
            Ok(Some(Event::SetParams(_parse_set_params_instruction(instruction, context, set_params)?)))
        },
        PumpfunInstruction::Create(create) => {
            Ok(Some(Event::Create(_parse_create_instruction(instruction, context, create)?)))
        },
        PumpfunInstruction::Buy(buy) => {
            Ok(Some(Event::Swap(_parse_buy_instruction(instruction, context, buy)?)))
        }
        PumpfunInstruction::Sell(sell) => {
            Ok(Some(Event::Swap(_parse_sell_instruction(instruction, context, sell)?)))
        }
        PumpfunInstruction::Withdraw => {
            Ok(Some(Event::Withdraw(_parse_withdraw_instruction(instruction, context)?)))
        }
        _ => Ok(None),
    }
}

fn _parse_initialize_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
) -> Result<InitializeEvent, Error> {
    let user = instruction.accounts()[0].to_string();

    Ok(InitializeEvent {
        user,
    })
}

fn _parse_set_params_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
    set_params: pumpfun::instruction::SetParamsInstruction,
) -> Result<SetParamsEvent, Error> {
    let user = instruction.accounts()[0].to_string();
    let fee_recipient = set_params.fee_recipient.to_string();
    let initial_virtual_token_reserves = set_params.initial_virtual_token_reserves;
    let initial_virtual_sol_reserves = set_params.initial_virtual_sol_reserves;
    let initial_real_token_reserves = set_params.initial_real_token_reserves;
    let token_total_supply = set_params.token_total_supply;
    let fee_basis_points = set_params.fee_basis_points;

    Ok(SetParamsEvent {
        user,
        fee_recipient,
        initial_virtual_token_reserves,
        initial_virtual_sol_reserves,
        initial_real_token_reserves,
        token_total_supply,
        fee_basis_points,
    })
}

fn _parse_create_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
    create: pumpfun::instruction::CreateInstruction,
) -> Result<CreateEvent, Error> {
    let user = instruction.accounts()[7].to_string();
    let name = create.name;
    let symbol = create.symbol;
    let uri = create.uri;
    let mint = instruction.accounts()[0].to_string();
    let bonding_curve = instruction.accounts()[2].to_string();
    let associated_bonding_curve = instruction.accounts()[2].to_string();
    let metadata = instruction.accounts()[6].to_string();

    Ok(CreateEvent {
        user,
        name,
        symbol,
        uri,
        mint,
        bonding_curve,
        associated_bonding_curve,
        metadata,
    })
}

fn _parse_buy_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
    buy: pumpfun::instruction::BuyInstruction,
) -> Result<SwapEvent, Error> {
    let mint = instruction.accounts()[2].to_string();
    let bonding_curve = instruction.accounts()[3].to_string();
    let user = instruction.accounts()[6].to_string();
    let token_amount = buy.amount;

    let system_transfer_instruction = instruction.inner_instructions()
        .iter()
        .find(|x| x.program_id() == SYSTEM_PROGRAM_ID)
        .ok_or(anyhow::anyhow!("No instruction with program_id == SYSTEM_PROGRAM_ID found"))?
        .clone();

    let system_transfer = system_program_substream::parse_transfer_instruction(system_transfer_instruction.as_ref(), context)?;
    let sol_amount = Some(system_transfer.lamports);

    let token_transfer_instruction = instruction.inner_instructions().iter().find(|x| x.program_id() == TOKEN_PROGRAM_ID).unwrap().clone();
    let token_transfer = spl_token_substream::parse_transfer_instruction(token_transfer_instruction.as_ref(), context).map_err(|e| anyhow!(e))?;
    let user_token_pre_balance = token_transfer.destination.unwrap().pre_balance;

    let trade = match parse_pumpfun_log(instruction) {
        Ok(PumpfunLog::Trade(trade)) => Some(trade),
        _ => None,
    };
    let virtual_sol_reserves = trade.as_ref().map(|x| x.virtual_sol_reserves);
    let virtual_token_reserves = trade.as_ref().map(|x| x.virtual_token_reserves);
    let real_sol_reserves = trade.as_ref().map(|x| x.real_sol_reserves);
    let real_token_reserves = trade.as_ref().map(|x| x.real_token_reserves);

    let direction = "token".to_string();

    Ok(SwapEvent {
        user,
        mint,
        bonding_curve,
        sol_amount,
        token_amount,
        direction,
        virtual_sol_reserves,
        virtual_token_reserves,
        real_sol_reserves,
        real_token_reserves,
        user_token_pre_balance,
    })
}

fn _parse_sell_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
    sell: pumpfun::instruction::SellInstruction,
) -> Result<SwapEvent, Error> {
    let mint = instruction.accounts()[2].to_string();
    let user = instruction.accounts()[6].to_string();
    let bonding_curve = instruction.accounts()[3].to_string();
    let token_amount = sell.amount;

    let trade = match parse_pumpfun_log(instruction) {
        Ok(PumpfunLog::Trade(trade)) => Some(trade),
        _ => None
    };
    let sol_amount = trade.as_ref().map(|x| x.sol_amount);
    let virtual_sol_reserves = trade.as_ref().map(|x| x.virtual_sol_reserves);
    let virtual_token_reserves = trade.as_ref().map(|x| x.virtual_token_reserves);
    let real_sol_reserves = trade.as_ref().map(|x| x.real_sol_reserves);
    let real_token_reserves = trade.as_ref().map(|x| x.real_token_reserves);

    let direction = "sol".to_string();

    let token_transfer_instruction = instruction.inner_instructions().iter().find(|x| x.program_id() == TOKEN_PROGRAM_ID).unwrap().clone();
    let token_transfer = spl_token_substream::parse_transfer_instruction(token_transfer_instruction.as_ref(), context).map_err(|e| anyhow!(e))?;
    let user_token_pre_balance = token_transfer.source.unwrap().pre_balance;

    Ok(SwapEvent {
        user,
        mint,
        bonding_curve,
        token_amount,
        sol_amount,
        direction,
        virtual_sol_reserves,
        virtual_token_reserves,
        real_sol_reserves,
        real_token_reserves,
        user_token_pre_balance,
    })
}

fn _parse_withdraw_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
) -> Result<WithdrawEvent, Error> {
    let mint = instruction.accounts()[2].to_string();

    Ok(WithdrawEvent {
        mint,
    })
}

fn parse_pumpfun_log(instruction: &StructuredInstruction) -> Result<PumpfunLog, Error> {
    let data = instruction.logs().as_ref().context("Failed to parse logs due to truncation")?.iter().find_map(|log| match log {
        Log::Data(data_log) => data_log.data().ok(),
        _ => None,
    }).ok_or(anyhow!("Couldn't find data log."))?;
    PumpfunLog::unpack(data.as_slice()).map_err(|x| anyhow!(x))
}
