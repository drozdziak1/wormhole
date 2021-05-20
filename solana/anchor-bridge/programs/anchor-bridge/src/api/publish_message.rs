use anchor_lang::{prelude::*, solana_program};

use crate::{
    accounts,
    anchor_bridge::Bridge,
    types::{BridgeConfig, Chain, Index},
    ErrorCode,
    PostedMessage,
    PublishMessage,
    Result,
    MAX_LEN_GUARDIAN_KEYS,
    VAA_TX_FEE,
};

/// Maximum size of a posted VAA
pub const MAX_PAYLOAD_SIZE: usize = 400;

pub fn publish_message(
    bridge: &mut Bridge,
    ctx: Context<PublishMessage>,
    nonce: u8,
    message_nonce: u32,
    payload: Vec<u8>,
) -> Result<()> {
    // Check that within this same transaction, the user paid the fee.
    check_fees(
        &ctx.accounts.instructions,
        &ctx.accounts.state,
        calculate_transfer_fee(),
    )?;

    // Manually create message account, as Anchor can't do it.
    let mut message: ProgramAccount<PostedMessage> = {
        // First create the message account. 8 Bytes additional for the discriminator.
        let space = 8 + PostedMessage::default().try_to_vec().unwrap().len();
        let lamports = ctx.accounts.rent.minimum_balance(space);
        let ix = solana_program::system_instruction::create_account(
            ctx.accounts.payer.key,
            ctx.accounts.message.key,
            lamports,
            space as u64,
            ctx.program_id,
        );

        // Derived seeds for a message account.
        let seeds = [
            ctx.program_id.as_ref(),
            ctx.accounts.emitter.key.as_ref(),
            &[nonce],
        ];

        // Wrap seeds in a signer list.
        let signer = &[&seeds[..]];

        // Create account using generated data.
        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.emitter.clone(),
                ctx.accounts.system_program.clone(),
            ],
            signer,
        )?;

        // Deserialize the newly created account into an object.
        ProgramAccount::try_from_init(&ctx.accounts.message)?
    };

    // Initialize Message data.
    message.submission_time = ctx.accounts.clock.unix_timestamp as u32;
    message.emitter_chain = Chain::Solana;
    message.emitter_address = ctx.accounts.emitter.key.to_bytes();
    message.nonce = message_nonce;
    message.payload = payload;

    // Manually persist changes since we manually created the account.
    message.exit(ctx.program_id)?;

    Ok(())
}

/// A const time calculation of the fee required to publish a message. Cost breakdown:
/// - 2 Signatures
/// - 1 Claimed VAA Rent
/// - 2x Guardian Fees
const fn calculate_transfer_fee() -> u64 {
    use std::mem::size_of;
    const SIGNATURE_COST: u64 = size_of::<SignatureState>() as u64;
    const VAA_COST: u64 = size_of::<ClaimedVAA>() as u64;
    const VAA_FEE: u64 = VAA_TX_FEE;
    SIGNATURE_COST + VAA_COST + VAA_FEE
}

/// Signature state
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SignatureState {
    /// signatures of validators
    pub signatures: [[u8; 65]; MAX_LEN_GUARDIAN_KEYS],

    /// hash of the data
    pub hash: [u8; 32],

    /// index of the guardian set
    pub guardian_set_index: u32,
}

/// Record of a claimed VAA
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClaimedVAA {
    /// hash of the vaa
    pub hash: [u8; 32],

    /// time the vaa was submitted
    pub vaa_time: u32,
}

/// Check Fees.
fn check_fees(instructions: &AccountInfo, bridge: &ProgramState<Bridge>, fee: u64) -> Result<()> {
    use solana_program::sysvar::instructions;

    let current_instruction = instructions::load_current_index(
        &instructions.try_borrow_mut_data()?,
    );
    if current_instruction == 0 {
        return Err(ProgramError::InvalidInstructionData.into());
    }

    // The previous ix must be a transfer instruction
    let transfer_ix_index = (current_instruction - 1) as u8;
    let transfer_ix = instructions::load_instruction_at(
        transfer_ix_index as usize,
        &instructions.try_borrow_mut_data()?,
    )
    .map_err(|_| ProgramError::InvalidAccountData)?;

    // Check that the instruction is actually for the system program
    if transfer_ix.program_id != solana_program::system_program::id() {
        return Err(ProgramError::InvalidArgument.into());
    }

    if transfer_ix.accounts.len() != 2 {
        return Err(ProgramError::InvalidInstructionData.into());
    }

    // Check that the fee was transferred to the bridge config.
    // We only care that the fee was sent to the bridge, not by whom it was sent.
    if transfer_ix.accounts[1].pubkey != *bridge.to_account_info().key {
        return Err(ProgramError::InvalidArgument.into());
    }

    // The transfer instruction is serialized using bincode (little endian)
    // uint32 ix_type = 2 (Transfer)
    // uint64 lamports
    // LEN: 4 + 8 = 12 bytes
    if transfer_ix.data.len() != 12 {
        return Err(ProgramError::InvalidAccountData.into());
    }

    // Verify action
    if transfer_ix.data[..4] != [2, 0, 0, 0] {
        return Err(ProgramError::InvalidInstructionData.into());
    }

    // Parse amount
    let mut fixed_data = [0u8; 8];
    fixed_data.copy_from_slice(&transfer_ix.data[4..]);
    let amount = u64::from_le_bytes(fixed_data);

    // Verify fee amount
    if amount < fee {
        return Err(ErrorCode::InsufficientFees.into());
    }

    Ok(())
}
