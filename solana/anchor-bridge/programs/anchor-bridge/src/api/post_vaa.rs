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
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Write, Cursor};
use sha3::Digest;

#[access_control(check_active(&ctx.accounts.guardian_set, &ctx.accounts.clock))]
#[access_control(check_valid_sigs(&ctx.accounts.guardian_set, &ctx.accounts.sig_info))]
#[access_control(check_integrity(&ctx.accounts.sig_info, &vaa))]
pub fn post_vaa(bridge: &mut Bridge, ctx: Context<PostVAA>, vaa: &PostVAAData) -> Result<()> {
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
