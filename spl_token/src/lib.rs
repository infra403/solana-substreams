use anyhow::{anyhow, Context, Error};

use substreams_solana::pb::sf::solana::r#type::v1::ConfirmedTransaction;

use substreams_solana_utils as utils;
use utils::instruction::{get_structured_instructions, StructuredInstruction, StructuredInstructions};
use utils::transaction::{get_context, TransactionContext};
use utils::spl_token::{TokenInstruction, TOKEN_PROGRAM_ID};
use utils::pubkey::Pubkey;

pub mod pb;
use pb::spl_token::*;
use pb::spl_token::spl_token_event::Event;

pub fn parse_transaction(transaction: &ConfirmedTransaction) -> Result<Vec<SplTokenEvent>, Error> {
    if let Some(_) = transaction.meta.as_ref().unwrap().err {
        return Ok(Vec::new())
    }

    let mut events: Vec<SplTokenEvent> = Vec::new();

    let mut context = get_context(transaction)?;
    let instructions = get_structured_instructions(transaction)?;

    for instruction in instructions.flattened().iter() {
        context.update_balance(&instruction.instruction);
        if instruction.program_id() == TOKEN_PROGRAM_ID {
            let event = parse_instruction(instruction, &context)?;
            events.push(SplTokenEvent { event });
        }
    }

    Ok(events)
}

pub fn parse_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<Option<Event>, Error> {
    if instruction.program_id() != TOKEN_PROGRAM_ID {
        return Err(anyhow!("Not a Token program instruction"));
    }

    let unpacked = TokenInstruction::unpack(&instruction.data())
        .map_err(|x| anyhow!(x).context("Failed to unpack Token instruction"))?;
    match unpacked {
        TokenInstruction::InitializeMint { decimals, mint_authority, freeze_authority } |
        TokenInstruction::InitializeMint2 { decimals, mint_authority, freeze_authority } => {
            let event = _parse_initialize_mint_instruction(instruction, context, decimals as u32, mint_authority, freeze_authority);
            event.map(|x| Some(Event::InitializeMint(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::InitializeAccount => {
            let event = _parse_initialize_account_instruction(instruction, context, None);
            event.map(|x| Some(Event::InitializeAccount(x))).map_err(|x| anyhow!(x))
        },
        TokenInstruction::InitializeAccount2 { owner } |
        TokenInstruction::InitializeAccount3 { owner } => {
            let event = _parse_initialize_account_instruction(instruction, context, Some(owner));
            event.map(|x| Some(Event::InitializeAccount(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::InitializeMultisig { m } => {
            let event = _parse_initialize_multisig_instruction(instruction, context, m, true);
            event.map(|x| Some(Event::InitializeMultisig(x))).map_err(|x| anyhow!(x))
        }
        TokenInstruction::InitializeMultisig2 { m } => {
            let event = _parse_initialize_multisig_instruction(instruction, context, m, false);
            event.map(|x| Some(Event::InitializeMultisig(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::Transfer { amount } => {
            let event = _parse_transfer_instruction(instruction, context, amount, None);
            event.map(|x| Some(Event::Transfer(x))).map_err(|x| anyhow!(x))
        },
        TokenInstruction::TransferChecked { amount, decimals } => {
            let event = _parse_transfer_instruction(instruction, context, amount, Some(decimals));
            event.map(|x| Some(Event::Transfer(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::Approve { amount } => {
            let event = _parse_approve_instruction(instruction, context, amount, None);
            event.map(|x| Some(Event::Approve(x))).map_err(|x| anyhow!(x))
        },
        TokenInstruction::ApproveChecked { amount, decimals } => {
            let event = _parse_approve_instruction(instruction, context, amount, Some(decimals));
            event.map(|x| Some(Event::Approve(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::Revoke => {
            let event = _parse_revoke_instruction(instruction, context);
            event.map(|x| Some(Event::Revoke(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::SetAuthority { authority_type, new_authority } => {
            let event = _parse_set_authority_instruction(instruction, context, authority_type, new_authority);
            event.map(|x| Some(Event::SetAuthority(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::MintTo { amount } => {
            let event = _parse_mint_to_instruction(instruction, context, amount);
            event.map(|x| Some(Event::MintTo(x))).map_err(|x| anyhow!(x))
        },
        TokenInstruction::MintToChecked { amount, decimals: _ } => {
            let event = _parse_mint_to_instruction(instruction, context, amount);
            event.map(|x| Some(Event::MintTo(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::Burn { amount } => {
            let event = _parse_burn_instruction(instruction, context, amount);
            event.map(|x| Some(Event::Burn(x))).map_err(|x| anyhow!(x))
        },
        TokenInstruction::BurnChecked { amount, decimals: _ } => {
            let event = _parse_burn_instruction(instruction, context, amount);
            event.map(|x| Some(Event::Burn(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::CloseAccount => {
            let event = _parse_close_account_instruction(instruction, context);
            event.map(|x| Some(Event::CloseAccount(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::FreezeAccount => {
            let event = _parse_freeze_account_instruction(instruction, context);
            event.map(|x| Some(Event::FreezeAccount(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::ThawAccount => {
            let event = _parse_thaw_account_instruction(instruction, context);
            event.map(|x| Some(Event::ThawAccount(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::InitializeImmutableOwner => {
            let event = _parse_initialize_immutable_owner_instruction(instruction, context);
            event.map(|x| Some(Event::InitializeImmutableOwner(x))).map_err(|x| anyhow!(x))
        },

        TokenInstruction::SyncNative => {
            let event = _parse_sync_native_instruction(instruction, context);
            event.map(|x| Some(Event::SyncNative(x))).map_err(|x| anyhow!(x))
        },
        TokenInstruction::AmountToUiAmount { amount: _ } => Ok(None),
        TokenInstruction::GetAccountDataSize => Ok(None),
        TokenInstruction::UiAmountToAmount { ui_amount: _ } => Ok(None),
    }.context("Failed to parse Token instruction")
}

fn _parse_initialize_mint_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
    decimals: u32,
    mint_authority: Pubkey,
    freeze_authority: Option<Pubkey>,
) -> Result<InitializeMintEvent, &'static str> {
    let mint = instruction.accounts()[0].to_string();
    let mint_authority = mint_authority.to_string();
    let freeze_authority = freeze_authority.map(|x| x.to_string());

    Ok(InitializeMintEvent {
        mint,
        decimals,
        mint_authority,
        freeze_authority,
    })
}

fn _parse_initialize_account_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
    _owner: Option<Pubkey>,
) -> Result<InitializeAccountEvent, &'static str> {
    let address = &instruction.accounts()[0];
    let token_account = context.get_token_account(address).unwrap();

    Ok(InitializeAccountEvent {
        account: Some(token_account.into())
    })
}

fn _parse_initialize_multisig_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
    m: u8,
    rent_sysvar_account: bool,
) -> Result<InitializeMultisigEvent, &'static str> {
    let multisig = instruction.accounts()[0].to_string();
    let mut signers: Vec<String> = Vec::new();
    let delta = if rent_sysvar_account { 2 } else { 1 };
    for account in instruction.accounts()[delta..].iter() {
        signers.push(account.to_string());
    }

    Ok(InitializeMultisigEvent {
        multisig,
        signers,
        m: m.into(),
    })
}

fn _parse_transfer_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
    amount: u64,
    expected_decimals: Option<u8>,
) -> Result<TransferEvent, &'static str> {
    let delta: usize = if expected_decimals.is_none() { 0 } else { 1 };
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();
    let destination = context.get_token_account(&instruction.accounts()[1 + delta]).unwrap();
    let authority = instruction.accounts()[2 + delta].to_string();

    Ok(TransferEvent {
        source: Some(source.into()),
        destination: Some(destination.into()),
        amount,
        authority,
    })
}

fn _parse_approve_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
    amount: u64,
    expected_decimals: Option<u8>,
) -> Result<ApproveEvent, &'static str> {
    let delta: usize = if expected_decimals.is_none() { 0 } else { 1 };
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();
    let delegate = instruction.accounts()[1 + delta].to_string();

    Ok(ApproveEvent {
        source: Some(source.into()),
        delegate,
        amount,
    })
}

fn _parse_revoke_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
) -> Result<RevokeEvent, &'static str> {
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();

    Ok(RevokeEvent {
        source: Some(source.into()),
    })
}

fn _parse_set_authority_instruction(
    instruction: &StructuredInstruction,
    _context: &TransactionContext,
    authority_type: utils::spl_token::AuthorityType,
    new_authority: Option<Pubkey>,
) -> Result<SetAuthorityEvent, &'static str> {
    let mint = instruction.accounts()[0].to_string();
    let authority = instruction.accounts()[1].to_string();
    let authority_type: i32 = match authority_type {
        utils::spl_token::AuthorityType::MintTokens => AuthorityType::MintTokens.into(),
        utils::spl_token::AuthorityType::FreezeAccount => AuthorityType::FreezeAccount.into(),
        utils::spl_token::AuthorityType::AccountOwner => AuthorityType::AccountOwner.into(),
        utils::spl_token::AuthorityType::CloseAccount => AuthorityType::CloseAccount.into(),
    };
    let new_authority = new_authority.map(|x| x.to_string());

    Ok(SetAuthorityEvent {
        mint,
        authority,
        authority_type,
        new_authority,
    })
}

fn _parse_mint_to_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
    amount: u64,
) -> Result<MintToEvent, &'static str> {
    let mint = instruction.accounts()[0].to_string();
    let destination = context.get_token_account(&instruction.accounts()[1]).unwrap();
    let mint_authority = instruction.accounts()[2].to_string();

    Ok(MintToEvent {
        mint,
        destination: Some(destination.into()),
        mint_authority,
        amount,
    })
}

fn _parse_burn_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
    amount: u64,
) -> Result<BurnEvent, &'static str> {
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();
    let _mint = instruction.accounts()[1].to_string();
    let authority = instruction.accounts()[2].to_string();

    Ok(BurnEvent {
        source: Some(source.into()),
        authority,
        amount,
    })
}

fn _parse_close_account_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
) -> Result<CloseAccountEvent, &'static str> {
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();
    let destination = instruction.accounts()[1].to_string();

    Ok(CloseAccountEvent {
        source: Some(source.into()),
        destination,
    })
}

fn _parse_freeze_account_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
) -> Result<FreezeAccountEvent, &'static str> {
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();
    let freeze_authority = instruction.accounts()[1].to_string();

    Ok(FreezeAccountEvent {
        source: Some(source.into()),
        freeze_authority,
    })
}

fn _parse_thaw_account_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
) -> Result<ThawAccountEvent, &'static str> {
    let source = context.get_token_account(&instruction.accounts()[0]).unwrap();
    let freeze_authority = instruction.accounts()[1].to_string();

    Ok(ThawAccountEvent {
        source: Some(source.into()),
        freeze_authority,
    })
}

fn _parse_initialize_immutable_owner_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
) -> Result<InitializeImmutableOwnerEvent, &'static str> {
    let account = context.get_token_account(&instruction.accounts()[0]).unwrap();

    Ok(InitializeImmutableOwnerEvent {
        account: Some(account.into()),
    })
}

fn _parse_sync_native_instruction(
    instruction: &StructuredInstruction,
    context: &TransactionContext,
) -> Result<SyncNativeEvent, &'static str> {
    let account = context.get_token_account(&instruction.accounts()[0]).unwrap();

    Ok(SyncNativeEvent {
        account: Some(account.into())
    })
}

pub fn parse_initialize_mint_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<InitializeMintEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::InitializeMint(initialize_mint))) => Ok(initialize_mint),
        _ => Err("Failed to parse initialize mint instruction."),
    }
}

pub fn parse_initialize_account_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<InitializeAccountEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::InitializeAccount(initialize_account))) => Ok(initialize_account),
        _ => Err("Failed to parse initialize account instruction."),
    }
}

pub fn parse_initialize_multisig_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<InitializeMultisigEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::InitializeMultisig(initialize_multisig))) => Ok(initialize_multisig),
        _ => Err("Failed to parse initialize multisig instruction."),
    }
}


pub fn parse_transfer_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<TransferEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::Transfer(transfer))) => Ok(transfer),
        _ => Err("Failed to parse transfer instruction."),
    }
}

pub fn parse_approve_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<ApproveEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::Approve(approve))) => Ok(approve),
        _ => Err("Failed to parse approve instruction."),
    }
}

pub fn parse_revoke_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<RevokeEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::Revoke(revoke))) => Ok(revoke),
        _ => Err("Failed to parse revoke instruction."),
    }
}

pub fn parse_set_authority_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<SetAuthorityEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::SetAuthority(set_authority))) => Ok(set_authority),
        _ => Err("Failed to parse set authority instruction."),
    }
}

pub fn parse_mint_to_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<MintToEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::MintTo(mint_to))) => Ok(mint_to),
        _ => Err("Failed to parse mint to instruction."),
    }
}

pub fn parse_burn_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<BurnEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::Burn(burn))) => Ok(burn),
        _ => Err("Failed to parse burn instruction."),
    }
}


pub fn parse_close_account_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<CloseAccountEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::CloseAccount(close_account))) => Ok(close_account),
        _ => Err("Failed to parse close account instruction."),
    }
}

pub fn parse_freeze_account_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<FreezeAccountEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::FreezeAccount(freeze_account))) => Ok(freeze_account),
        _ => Err("Failed to parse freeze account instruction."),
    }
}

pub fn parse_thaw_account_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<ThawAccountEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::ThawAccount(thaw_account))) => Ok(thaw_account),
        _ => Err("Failed to parse thaw account instruction."),
    }
}

pub fn parse_initialize_immutable_owner_instruction<'a>(
    instruction: &StructuredInstruction<'a>,
    context: &TransactionContext,
) -> Result<InitializeImmutableOwnerEvent, &'static str> {
    match parse_instruction(instruction, context) {
        Ok(Some(Event::InitializeImmutableOwner(initialize_immutable_owner))) => Ok(initialize_immutable_owner),
        _ => Err("Failed to parse initialize immutable owner instruction."),
    }
}

impl<'a> From<&'a utils::spl_token::TokenAccount<'a>> for TokenAccount {
    fn from(value: &'a utils::spl_token::TokenAccount<'a>) -> Self {
        Self {
            address: value.address.to_string(),
            owner: value.owner.to_string(),
            mint: value.mint.to_string(),
            pre_balance: value.pre_balance,
            post_balance: value.post_balance,
        }
    }
}
