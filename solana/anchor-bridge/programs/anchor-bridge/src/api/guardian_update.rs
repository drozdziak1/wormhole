use anchor_lang::{prelude::*, solana_program};

use crate::{
    accounts,
    anchor_bridge::Bridge,
    types::{BridgeConfig, Index},
    GuardianUpdate,
    GuardianUpdateData,
    Result,
    MAX_LEN_GUARDIAN_KEYS,
};

pub fn guardian_update(
    bridge: &mut Bridge,
    ctx: Context<GuardianUpdate>,
    data: GuardianUpdateData,
) -> Result<()> {
    Ok(())
}
