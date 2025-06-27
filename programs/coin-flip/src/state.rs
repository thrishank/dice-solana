use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
pub enum BetType {
    Over,
    Under,
}

#[account]
pub struct PlayerState {
    pub allowed_user: Pubkey,
    pub randomness_account: Pubkey, // Reference to the Switchboard randomness account
    pub current_guess: u8,          // The current guess
    pub wager: u64,                 // The wager amount
    pub bet_type: BetType,          // The type of bet (Over/Under)
    pub commit_slot: u64,           // The slot at which the randomness was committed
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Treasury {
    pub bump: u8,
    pub owner: Pubkey,
}
