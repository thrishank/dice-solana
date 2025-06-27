use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized access attempt")]
    Unauthorized,

    #[msg("Invalid guess value. Must be 0-127")]
    InvalidGuess,

    #[msg("Invalid bet amount. Must be greater than 0")]
    InvalidBetAmount,

    #[msg("Bet amount out of allowed range")]
    BetOutOfRange,

    #[msg("Randomness data already revealed or invalid")]
    RandomnessAlreadyRevealed,

    #[msg("Randomness not yet resolved")]
    RandomnessNotResolved,

    #[msg("Randomness data expired")]
    RandomnessExpired,

    #[msg("Invalid randomness account")]
    InvalidRandomnessAccount,

    #[msg("Failed to parse randomness data")]
    InvalidRandomnessData,

    #[msg("Invalid withdrawal amount")]
    InvalidAmount,

    #[msg("Withdrawal would leave treasury below rent exemption")]
    InsufficientRentBalance,

    #[msg("Insufficient treasury balance for withdrawal")]
    InsufficientTreasuryBalance,

    #[msg("Withdrawal limit exceeded. Maximum is 10 SOL")]
    WithdrawalLimitExceeded,

    #[msg("Insufficient treasury funds to pay winner")]
    InsufficientTreasuryFunds,

    MathOverflow,
}
