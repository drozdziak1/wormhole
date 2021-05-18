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
    MAX_LEN_GUARDIAN_KEYS,
};

#[access_control(check_active(&ctx.accounts.guardian_set, &ctx.accounts.clock))]
#[access_control(check_valid_sigs(&ctx.accounts.guardian_set, &ctx.accounts.sig_info))]
pub fn post_vaa(bridge: &mut Bridge, ctx: Context<PostVAA>) -> Result<()> {
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
fn check_valid_sigs(guardian_set: &GuardianSetInfo, sig_info: &AccountInfo) -> Result<()> {
    if sig_state.guardian_set_index != guardian_set.index {
        return Err(ErrorCode::PostVAAGuardianSetMismatch.into());
    }
    Ok(())
}
