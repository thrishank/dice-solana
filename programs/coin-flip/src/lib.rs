use anchor_lang::prelude::*;
use anchor_lang::system_program;
use switchboard_on_demand::accounts::RandomnessAccountData;

mod error;
mod state;

use crate::state::PlayerState;
use crate::state::Treasury;
use crate::{state::BetType};

declare_id!("8bAReNo1WM2oRuRucz3cUfzs6B3j6oVL4S22QQ8WdN3m");

#[program]
pub mod coin_flip {

    use super::*;

    pub fn dice_roll(ctx: Context<DiceRoll>, _id: u64, guess: u8, bet: u64, bet_type: BetType) -> Result<()> {
        // Validate inputs
        require!((2..=98).contains(&guess), error::ErrorCode::InvalidGuess);

        const MIN_BET: u64 = 1_000_000; // 0.001 SOL minimum
        const MAX_BET: u64 = 1_000_000_000; // 1 SOL maximum
        require!(
            (MIN_BET..=MAX_BET).contains(&bet),
            error::ErrorCode::BetOutOfRange
        );

        let clock = Clock::get()?;
        let player_state = &mut ctx.accounts.player_state;

        // Initialize player state
        player_state.current_guess = guess;
        player_state.bump = ctx.bumps.player_state;
        player_state.allowed_user = ctx.accounts.user.key();
        player_state.wager = bet;
        player_state.bet_type = bet_type;

        // Parse and validate randomness data
        let randomness_data =
            RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow())
                .unwrap();

        require!(
            randomness_data.seed_slot == clock.slot - 1,
            error::ErrorCode::RandomnessAlreadyRevealed
        );

        player_state.commit_slot = randomness_data.seed_slot;
        player_state.randomness_account = ctx.accounts.randomness_account_data.key();

        // Transfer bet amount to treasury
        let trasfer_sol_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.treasury.to_account_info(),
            },
        );

        system_program::transfer(trasfer_sol_cpi, bet)?;
        msg!(
            "Dice roll initiated, Guess: {}, Bet Type: {:?}, Bet: {} lamports",
            guess,
            bet_type,
            bet
        );
        Ok(())
    }

    pub fn settle_flip(ctx: Context<SettleFlip>, _id: u64) -> Result<()> {
        let clock: Clock = Clock::get()?;
        let player_state = &mut ctx.accounts.player_state;

        // Verify that the provided randomness account matches the stored one
        require!(
            ctx.accounts.randomness_account_data.key() == player_state.randomness_account,
            error::ErrorCode::InvalidRandomnessAccount
        );

        // parse the randomness account data
        let randomness_data =
            RandomnessAccountData::parse(ctx.accounts.randomness_account_data.data.borrow())
                .unwrap();

        require!(
            randomness_data.seed_slot == player_state.commit_slot,
            error::ErrorCode::RandomnessExpired
        );

        // get the random data
        let revealed_random_value = randomness_data
            .get_value(&clock)
            .map_err(|_| error::ErrorCode::RandomnessNotResolved)?;

        msg!("random data: {:?}", revealed_random_value);

        // Generate dice roll from random bytes (0-100)
        let dice_roll = generate_dice_roll(&revealed_random_value);
        msg!("Dice roll: {}", dice_roll);

        // Calculate if player won
        let player_won = match player_state.bet_type {
            BetType::Over => dice_roll > player_state.current_guess,
            BetType::Under => dice_roll < player_state.current_guess,
        }; 
        
        if player_won {
            // Calculate payout with house edge
            let payout = calculate_payout(player_state.wager, player_state.current_guess, player_state.bet_type)?;

            let rent = Rent::get()?;
            let min_balance = rent.minimum_balance(ctx.accounts.treasury.to_account_info().data_len());
            let available_balance = ctx.accounts.treasury.to_account_info().lamports()
                .saturating_sub(min_balance);

             if payout > available_balance {
                msg!("Insufficient treasury funds. Available: {}, Required: {}", 
                     available_balance, payout);
                return Err(error::ErrorCode::InsufficientTreasuryFunds.into());
            }

            **ctx.accounts.treasury.to_account_info().try_borrow_mut_lamports()? -= payout;
            **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += payout;
            msg!(
                "Player won! Dice roll: {}, Guess: {}, Bet type: {:?}, Payout: {} lamports", 
                dice_roll, 
                player_state.current_guess,
                player_state.bet_type,
                payout
            );
        } else {
            msg!(
                "Player lost. Dice roll: {}, Guess: {}, Bet type: {:?}", 
                dice_roll, 
                player_state.current_guess,
                player_state.bet_type
            );
        }

        Ok(())
    }

    pub fn initialize_treasury(ctx: Context<InitTreasury>) -> Result<()> {
        ctx.accounts.treasury.set_inner(Treasury {
            bump: ctx.bumps.treasury,
            owner: ctx.accounts.signer.key(),
        });

        msg!("Treasury initialized with owner: {}", ctx.accounts.signer.key());

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let treasury = &mut ctx.accounts.treasury;

        if ctx.accounts.signer.key() != treasury.owner.key() {
            return Err(error::ErrorCode::Unauthorized.into());
        }

        if amount == 0 {
            return Err(error::ErrorCode::InvalidAmount.into());
        }

        let rent = Rent::get()?;
        let min_rent = rent.minimum_balance(treasury.to_account_info().data_len());
        let remaining_balance = treasury.to_account_info().lamports().saturating_sub(amount);

        if remaining_balance < min_rent {
            msg!("Withdrawal would leave treasury below rent exemption threshold");
            return Err(error::ErrorCode::InsufficientRentBalance.into());
        }

        const MAX_WITHDRAWAL: u64 = 10_000_000_000; // 10 SOL in lamports
        if amount > MAX_WITHDRAWAL {
            return Err(error::ErrorCode::WithdrawalLimitExceeded.into());
        }

        let transfer_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: treasury.to_account_info(),
                to: ctx.accounts.signer.to_account_info(),
            },
        );

        system_program::transfer(transfer_cpi, amount)?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(_id: u64)]
pub struct DiceRoll<'info> {
    #[account(
        init,
        payer = user,
        seeds = [b"player_state".as_ref(), _id.to_le_bytes().as_ref(), user.key().as_ref()],
        space = 8 + 120,
        bump
    )]
    pub player_state: Account<'info, PlayerState>,

    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,

    #[account(mut, seeds = [b"treasury"], bump)]
    pub treasury: Account<'info, Treasury>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(_id: u64)]
pub struct SettleFlip<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut,
        seeds = [b"player_state".as_ref(), _id.to_le_bytes().as_ref(), user.key().as_ref()],
        bump = player_state.bump,
        close = user,
    )]
    pub player_state: Account<'info, PlayerState>,

    /// CHECK: The account's data is validated manually within the handler.
    pub randomness_account_data: AccountInfo<'info>,

    #[account(mut, seeds = [b"treasury"], bump)]
    pub treasury: Account<'info, Treasury>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitTreasury<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init , 
        payer = signer,
        space = 8 + Treasury::INIT_SPACE,
        seeds = [b"treasury"],
        bump
    )]
    pub treasury: Account<'info, Treasury>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"treasury"],
        bump = treasury.bump,
    )]
    pub treasury: Account<'info, Treasury>,

    pub system_program: Program<'info, System>,
}

pub fn generate_dice_roll(random_bytes: &[u8]) -> u8 {
    // Use multiple bytes for better distribution
    let mut result: u32 = 0;
    for (i, &byte) in random_bytes.iter().take(4).enumerate() {
        result += (byte as u32) << (i * 8);
    }
    // Map to 0-100 range
    (result % 101) as u8
}

fn calculate_payout(wager: u64, guess: u8, bet_type: BetType) -> Result<u64> {
    require!((2..=98).contains(&guess), error::ErrorCode::InvalidGuess);

    // Calculate winning outcomes based on bet type
    let winning_outcomes = match bet_type {
        BetType::Over => 99 - guess as u64,  // Numbers greater than guess
        BetType::Under => (guess - 1) as u64,      // Numbers less than guess
    };

    let total_outcomes = 99u64;

    // Multiplier with 5% house edge
    let precision: u64 = 1_000_000; // 6 decimal fixed-point
    let house_edge: u64 = 950_000;  // 95% expressed as 950_000 / 1_000_000

    // Safe math using u128 to prevent overflow during intermediate steps
    let numerator = (total_outcomes as u128)
        .checked_mul(house_edge as u128)
        .ok_or(error::ErrorCode::MathOverflow)?;

    let denominator = winning_outcomes as u128;

    let multiplier_fp = numerator
        .checked_div(denominator)
        .ok_or(error::ErrorCode::MathOverflow)?;

    // Minimum multiplier is 1x, scaled by precision
    let multiplier_fp = std::cmp::max(multiplier_fp, precision as u128);
    
    // Final payout: wager * multiplier, adjusted by precision
    let payout = (wager as u128)
        .checked_mul(multiplier_fp)
        .and_then(|v| v.checked_div(precision as u128))
        .ok_or(error::ErrorCode::MathOverflow)?;

    msg!("multiplier: {}", multiplier_fp);
    Ok(payout as u64)
}
