#![allow(warnings)]

use anchor_lang::{prelude::*, solana_program};

pub mod api;
pub mod types;

use types::{Chain, Index};

pub use types::BridgeConfig;

// Without this, Anchor's derivation macros break. It requires names with no path components at all
// otherwise it errors.
use anchor_bridge::Bridge;

/// Chain ID of the chain this contract is deployed on.
pub const CHAIN_ID_SOLANA: u8 = Chain::Solana as u8;

/// Maximum number of guardians.
pub const MAX_LEN_GUARDIAN_KEYS: usize = 20;

/// Tx fee of Signature checks and PostVAA (see docs for calculation)
pub const VAA_TX_FEE: u64 = 18 * 10000;

#[derive(Accounts)]
pub struct VerifySig<'info> {
    /// Account used for paying auxillary transactions.
    #[account(signer)]
    pub payer_info: AccountInfo<'info>,

    /// The set of signatures we intend on verifying.
    pub signatures: ProgramAccount<'info, Signatures>,

    /// Guardian Set data used for verifying the signatures with.
    pub guardian_set_info: ProgramAccount<'info, GuardianSetInfo>,

    /// Instructions used for transaction reflection. Note that this should really be a
    /// Sysvar<'info, Instructions> but Solana has not implemented `Sysvar` for this type, so
    /// instead we have an AccountInfo and manually verify.
    ///
    /// https://github.com/solana-labs/solana/issues/17017
    pub instruction: AccountInfo<'info>,

    /// Required by Anchor for associated accounts.
    pub rent: Sysvar<'info, Rent>,

    /// Required by Anchor for associated accounts.
    pub system_program: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct VerifySigsData {
    /// hash of the VAA
    pub hash: [u8; 32],
    /// instruction indices of signers (-1 for missing)
    pub signers: [i8; MAX_LEN_GUARDIAN_KEYS],
    /// indicates whether this verification should only succeed if the sig account does not exist
    pub initial_creation: bool,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Account used to pay for auxillary instructions.
    #[account(signer)]
    pub payer: AccountInfo<'info>,

    /// Information about the current guardian set.
    // #[account(init, associated = state)]
    #[account(init)]
    pub guardian_set: ProgramAccount<'info, GuardianSetInfo>,

    /// State struct, derived by #[state], used for associated accounts.
    // pub state: ProgramState<'info, Bridge>,

    /// Used for timestamping actions.
    pub clock: Sysvar<'info, Clock>,

    /// Required by Anchor for associated accounts.
    pub rent: Sysvar<'info, Rent>,

    /// Required by Anchor for associated accounts.
    pub system_program: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeData {
    /// number of initial guardians
    pub len_guardians: u8,

    /// guardians that are allowed to sign mints
    pub initial_guardian_keys: Vec<[u8; 20]>,

    /// config for the bridge
    pub config: BridgeConfig,
}

#[derive(Accounts)]
pub struct PublishMessage<'info> {
    /// No need to verify - only used as the fee payer for account creation.
    #[account(signer)]
    pub payer: AccountInfo<'info>,

    /// The emitter, only used as metadata. We verify that the account is a signer to prevent
    /// messages from being spoofed.
    #[account(signer)]
    pub emitter: AccountInfo<'info>,

    /// The message account to store data in, note that this cannot be derived by serum and so the
    /// pulish_message handler does this by hand.
    pub message: AccountInfo<'info>,

    /// State struct, derived by #[state], used for associated accounts.
    pub state: ProgramState<'info, Bridge>,

    /// Instructions used for transaction reflection. Note that this should really be a
    /// Sysvar<'info, Instructions> but Solana has not implemented `Sysvar` for this type, so
    /// instead we have an AccountInfo and manually verify.
    ///
    /// https://github.com/solana-labs/solana/issues/17017
    pub instructions: AccountInfo<'info>,

    /// Clock used for timestamping.
    pub clock: Sysvar<'info, Clock>,

    /// Required by Anchor for associated accounts.
    pub rent: Sysvar<'info, Rent>,

    /// Required by Anchor for associated accounts.
    pub system_program: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PublishMessageData {
    /// Unique nonce for this message.
    pub nonce: u32,

    /// Message payload as an arbitrary string of bytes.
    pub payload: Vec<u8>,
}

#[derive(Accounts)]
pub struct PostVAA<'info> {
    /// Required by Anchor for associated accounts.
    pub system_program: AccountInfo<'info>,

    /// Required by Anchor for associated accounts.
    pub rent: Sysvar<'info, Rent>,

    /// Clock used for timestamping.
    pub clock: Sysvar<'info, Clock>,

    /// State struct, derived by #[state], used for associated accounts.
    pub state: ProgramState<'info, Bridge>,

    /// Information about the current guardian set.
    #[account(init, associated = state)]
    pub guardian_set: ProgramAccount<'info, GuardianSetInfo>,

    /// Bridge Info
    pub bridge_info: ProgramState<'info, BridgeInfo>,

    /// Claim Info
    pub claim: ProgramAccount<'info, ClaimedVAA>,

    /// Signature Info
    pub sig_info: ProgramAccount<'info, Signatures>,

    /// Account used to pay for auxillary instructions.
    #[account(signer)]
    pub payer: AccountInfo<'info>,

    /// Message the VAA is associated with.
    #[account(signer)]
    pub message: ProgramAccount<'info, PostedMessage>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Signature {
    pub index: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

pub type ForeignAddress = [u8; 32];

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PostVAAData {
    // Header part
    pub version: u8,
    pub guardian_set_index: u32,
    pub signatures: Vec<Signature>,

    // Body part
    pub timestamp: u32,
    pub nonce: u32,
    pub emitter_chain: u8,
    pub emitter_address: ForeignAddress,
    pub payload: Vec<u8>,
}

#[derive(Accounts)]
pub struct GuardianUpdate<'info> {
    /// Required by Anchor for associated accounts.
    pub system_program: AccountInfo<'info>,

    /// Required by Anchor for associated accounts.
    pub rent: Sysvar<'info, Rent>,

    /// Clock used for timestamping.
    pub clock: Sysvar<'info, Clock>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct GuardianUpdateData {
    pub dummy: u32,
}

#[program]
pub mod anchor_bridge {
    use super::*;

    #[state]
    pub struct Bridge {
        /// The current guardian set index, used to decide which signature sets to accept.
        pub guardian_set_index: types::Index,

        /// Bridge configuration, which is set once upon initialization.
        pub config: types::BridgeConfig,
    }

    impl Bridge {
        pub fn new(ctx: Context<Initialize>, data: InitializeData) -> Result<Self> {
	    msg!("Yeah boiii");
            api::initialize(
                ctx,
                data.len_guardians,
                data.initial_guardian_keys,
                data.config,
            )
        }

        pub fn publish_message(
            &mut self,
            ctx: Context<PublishMessage>,
            data: PublishMessageData,
            nonce: u8,
        ) -> Result<()> {
            // Sysvar trait not implemented for Instructions by sdk, so manual check required.  See
            // the VerifySig struct for more info.
            if *ctx.accounts.instructions.key != solana_program::sysvar::instructions::id() {
                return Err(ErrorCode::InvalidSysVar.into());
            }

            api::publish_message(
                self,
                ctx,
                nonce,
                data.nonce,
                data.payload,
            )
        }

        pub fn post_vaa(&mut self, ctx: Context<PostVAA>, data: PostVAAData) -> Result<()> {
            api::post_vaa(
                self,
                ctx,
                &data,
            )
        }

        pub fn verify_signatures(&mut self, ctx: Context<VerifySig>, data: VerifySigsData) -> Result<()> {
            // Sysvar trait not implemented for Instructions by sdk, so manual check required.  See
            // the VerifySig struct for more info.
            if *ctx.accounts.instruction.key != solana_program::sysvar::instructions::id() {
                return Err(ErrorCode::InvalidSysVar.into());
            }

            api::verify_signatures(
                self,
                ctx,
                data.hash,
                data.signers,
                data.initial_creation
            )
        }

        pub fn process_guardian_update(&mut self, ctx: Context<GuardianUpdate>, data: GuardianUpdateData) -> Result<()> {
	    msg!("Yeah boii in process_guardian_update()");
            api::guardian_update(
                self,
                ctx,
                data,
            )
        }
    }
}

#[error]
pub enum ErrorCode {
    #[msg("Error captured from io::Error")]
    IoError,

    #[msg("System account pubkey did not match expected address.")]
    InvalidSysVar,

    #[msg("Transaction did not transfer enough fees to suceed.")]
    InsufficientFees,

    #[msg("PostVAA cannot execute with an expired guardian set.")]
    PostVAAGuardianSetExpired,

    #[msg("PostVAA cannot execute with the wrong guardian set version.")]
    PostVAAGuardianSetMismatch,

    #[msg("PostVAA failed to due to not enough signatures required for consensus")]
    PostVAAConsensusFailed,
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        ErrorCode::IoError.into()
    }
}

#[account]
pub struct BridgeInfo {}

#[associated]
pub struct GuardianSetInfo {
    /// Version number of this guardian set.
    pub index: Index,

    /// public key hashes of the guardian set
    pub keys: Vec<[u8; 20]>,

    /// creation time
    pub creation_time: u32,

    /// expiration time when VAAs issued by this set are no longer valid
    pub expiration_time: u32,
}

/// Signatures contains metadata about signers in a VerifySignature ix
#[account]
pub struct Signatures {
    /// signatures of validators
    pub signatures: Vec<[u8; 32]>,

    /// hash of the data
    pub hash: [u8; 32],

    /// index of the guardian set
    pub guardian_set_index: Index,
}

/// Record of a posted wormhole message.
#[account]
#[derive(Default)]
pub struct PostedMessage {
    /// header of the posted VAA
    pub vaa_version: u8,

    /// time the vaa was submitted
    pub vaa_time: u32,

    /// Account where signatures are stored
    pub vaa_signature_account: Pubkey,

    /// time the posted message was created
    pub submission_time: u32,

    /// unique nonce for this message
    pub nonce: u32,

    /// emitter of the message
    pub emitter_chain: Chain,

    /// emitter of the message
    pub emitter_address: [u8; 32],

    /// message payload
    pub payload: Vec<u8>,
}

#[account]
#[derive(Default)]
pub struct ClaimedVAA {
    /// Hash of the VAA being claimed.
    pub hash: [u8; 32],

    /// Time the VAA claim was submitted.
    pub vaa_time: u32,
}
