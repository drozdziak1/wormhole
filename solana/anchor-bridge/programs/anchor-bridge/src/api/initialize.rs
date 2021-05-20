use anchor_lang::{prelude::*, solana_program};

use crate::{
    Result,
    accounts,
    anchor_bridge::Bridge,
    types::{BridgeConfig, Index},
    Initialize,
    InitializeData,
    MAX_LEN_GUARDIAN_KEYS,
};

#[access_control(check_keys(&initial_guardian_keys))]
pub fn initialize(
    ctx: Context<Initialize>,
    len_guardians: u8,
    initial_guardian_keys: Vec<[u8; 20]>,
    config: BridgeConfig,
) -> Result<Bridge> {
    let index = Index(0);

    // Initialize the Guardian Set for the first time.
    ctx.accounts.guardian_set.index = index;
    ctx.accounts.guardian_set.creation_time = ctx.accounts.clock.unix_timestamp as u32;
    ctx.accounts.guardian_set.keys = initial_guardian_keys;

    // Create an initial bridge state, labeled index 0.
    Ok(Bridge {
        guardian_set_index: index,
        config,
    })
}

/// Verify that the number of guardian keys passed is not more than the program defined maximum.
#[inline(always)]
fn check_keys(keys: &Vec<[u8; 20]>) -> Result<()> {
    if keys.len() > MAX_LEN_GUARDIAN_KEYS {
        return Err(ProgramError::InvalidInstructionData.into());
    }
    Ok(())
}
