use anchor_lang::{prelude::*, solana_program};

use crate::{
    accounts,
    anchor_bridge::Bridge,
    types::{BridgeConfig, Index},
    ErrorCode,
    GuardianSetInfo,
    PostVAA,
    PostVAAData,
    Result,
    Signatures,
    MAX_LEN_GUARDIAN_KEYS,
    VAA_TX_FEE,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use sha3::Digest;
use std::io::{Cursor, Write};

const MIN_BRIDGE_BALANCE: u64 = (((solana_program::rent::ACCOUNT_STORAGE_OVERHEAD
    + std::mem::size_of::<Bridge>() as u64)
    * solana_program::rent::DEFAULT_LAMPORTS_PER_BYTE_YEAR) as f64
    * solana_program::rent::DEFAULT_EXEMPTION_THRESHOLD) as u64;

#[access_control(check_active(&ctx.accounts.guardian_set, &ctx.accounts.clock))]
#[access_control(check_valid_sigs(&ctx.accounts.guardian_set, &ctx.accounts.sig_info))]
#[access_control(check_integrity(&ctx.accounts.sig_info, &vaa))]
pub fn post_vaa(bridge: &mut Bridge, ctx: Context<PostVAA>, vaa: &PostVAAData) -> Result<()> {
    // Count the numnber of signatures currently present.
    let signature_count = ctx
        .accounts
        .sig_info
        .signatures
        .iter()
        .filter(|v| v.iter().filter(|v| **v != 0).count() != 0)
        .count() as u8;

    // Calculate how many signatures are required to reach consensus. This calculation is in
    // expanded form to ease auditing.
    let required_consensus_count = {
        // Take key length as u16 to avoid overflow when multiplying by 10.
        let len = ctx.accounts.guardian_set.len_keys as u16;

        // Fixed point number transformation with one decimal to deal with rounding.
        let len = (len * 10) / 3;

        // Multiplication by two to get a 2/3 quorum.
        let len = len * 2;

        // Division by 10+1 to bring the number back into range.
        len / (10 + 1)
    };

    if (signature_count as u16) < required_consensus_count {
        return Err(ErrorCode::PostVAAConsensusFailed.into());
    }

    // Store VAA data in associated message.
    // TODO: Verify Body Message
    ctx.accounts.message.vaa_version = vaa.version;
    ctx.accounts.message.vaa_time = vaa.timestamp;
    ctx.accounts.message.vaa_signature_account = *ctx.accounts.sig_info.to_account_info().key;

    // If the bridge has enough balance, refund the SOL to the transaction payer.
    if VAA_TX_FEE
        < ctx
            .accounts
            .state
            .to_account_info()
            .lamports()
            .checked_sub(MIN_BRIDGE_BALANCE)
            .unwrap_or(0)
    {
        transfer_sol(
            &ctx.accounts.state.to_account_info(),
            &ctx.accounts.payer,
            VAA_TX_FEE,
        )?;
    }

    // Claim the VAA
    ctx.accounts.claim.vaa_time = ctx.accounts.clock.unix_timestamp as u32;

    Ok(())
}

fn transfer_sol(sender: &AccountInfo, recipient: &AccountInfo, amount: u64) -> Result<()> {
    let mut payer_balance = sender.try_borrow_mut_lamports()?;
    **payer_balance = payer_balance
        .checked_sub(amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    let mut recipient_balance = recipient.try_borrow_mut_lamports()?;
    **recipient_balance = recipient_balance
        .checked_add(amount)
        .ok_or(ProgramError::InvalidArgument)?;

    Ok(())
}

/// A guardian set must not have expired.
#[inline(always)]
fn check_active<'r>(guardian_set: &GuardianSetInfo, clock: &Sysvar<'r, Clock>) -> Result<()> {
    if guardian_set.expiration_time != 0
        && (guardian_set.expiration_time as i64) < clock.unix_timestamp
    {
        return Err(ErrorCode::PostVAAGuardianSetExpired.into());
    }

    Ok(())
}

/// The signatures in this instruction must be from the right guardian set.
#[inline(always)]
fn check_valid_sigs<'r>(
    guardian_set: &GuardianSetInfo,
    sig_info: &ProgramAccount<'r, Signatures>,
) -> Result<()> {
    if sig_info.guardian_set_index != guardian_set.index {
        return Err(ErrorCode::PostVAAGuardianSetMismatch.into());
    }
    Ok(())
}

#[inline(always)]
fn check_integrity<'r>(sig_info: &ProgramAccount<'r, Signatures>, vaa: &PostVAAData) -> Result<()> {
    let body = {
        let mut v = Cursor::new(Vec::new());
        v.write_u32::<BigEndian>(vaa.timestamp)?;
        v.write_u32::<BigEndian>(vaa.nonce)?;
        v.write_u8(vaa.emitter_chain)?;
        v.write(&vaa.emitter_address)?;
        v.write(&vaa.payload)?;
        v.into_inner()
    };

    let body_hash: [u8; 32] = {
        let mut h = sha3::Keccak256::default();
        h.write(body.as_slice()).map_err(|_| ProgramError::InvalidArgument);
        h.finalize().into()
    };

    Ok(())
}
