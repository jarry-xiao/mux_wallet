use anchor_lang::{
    prelude::*,
    solana_program::{
        program::{invoke, invoke_signed},
        system_instruction,
        sysvar::rent::Rent,
    },
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub fn assert_with_msg(statement: bool, err: ProgramError, msg: &str) -> ProgramResult {
    if !statement {
        msg!(msg);
        Err(err)
    } else {
        Ok(())
    }
}

pub fn print_dec(n_dec: u64, precision: u8) -> String {
    let base = 10_u64.pow(precision as u32);
    if base == 0 {
        return String::from("0");
    }
    let lhs = n_dec / base;
    let rhs = format!("{:0width$}", n_dec % base, width = precision as usize);
    format!("{}.{}", lhs, rhs)
}

#[program]
pub mod mux {
    use super::*;

    pub fn create_fund(ctx: Context<CreateFund>, total_shares: u64) -> ProgramResult {
        let bump = match ctx.bumps.get("fund_wallet") {
            Some(b) => *b,
            None => {
                msg!("Bump seed missing from ctx");
                return Err(ProgramError::InvalidArgument);
            }
        };
        let minimum_rent = Rent::get()?
            .minimum_balance(0)
            .saturating_sub(ctx.accounts.fund_wallet.lamports());
        if minimum_rent > 0 {
            msg!("Sending rent {} lamports", minimum_rent);
            invoke(
                &system_instruction::transfer(
                    &ctx.accounts.creator.key(),
                    &ctx.accounts.fund_wallet.key(),
                    minimum_rent,
                ),
                &[
                    ctx.accounts.creator.to_account_info(),
                    ctx.accounts.fund_wallet.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;
        }
        let excess = ctx
            .accounts
            .fund_wallet
            .lamports()
            .saturating_sub(Rent::get()?.minimum_balance(0));
        if excess > 0 {
            invoke_signed(
                &system_instruction::transfer(
                    &ctx.accounts.fund_wallet.key(),
                    &ctx.accounts.creator.key(),
                    excess,
                ),
                &[
                    ctx.accounts.fund_wallet.to_account_info(),
                    ctx.accounts.creator.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[&[ctx.accounts.wallet_state.key().as_ref(), &[bump]]],
            )?;
        }
        assert_with_msg(
            ctx.accounts.fund_wallet.lamports() == minimum_rent,
            ProgramError::AccountNotRentExempt,
            "Fund wallet must be rent exempt",
        )?;
        ctx.accounts.wallet_state.initialize(
            ctx.accounts.creator.key(),
            total_shares,
            ctx.accounts.fund_wallet.lamports(),
            bump,
        );
        ctx.accounts.creator_state.initialize(total_shares, 0);
        Ok(())
    }

    pub fn create_stake_account(ctx: Context<CreateStakeAccount>) -> ProgramResult {
        ctx.accounts
            .user_state
            .initialize(0, ctx.accounts.wallet_state.total_deposits_per_share);
        Ok(())
    }

    pub fn transfer_shares(ctx: Context<TransferShares>, num_shares: u64) -> ProgramResult {
        let wallet_key = ctx.accounts.wallet_state.key();
        let wallet_balance = ctx.accounts.fund_wallet.lamports();
        let wallet_state = &mut ctx.accounts.wallet_state;
        assert_with_msg(
            wallet_balance >= wallet_state.last_snapshot,
            ProgramError::InvalidAccountData,
            "Wallet cannot have less SOL than last snaphshot",
        )?;
        wallet_state.update_internal_accounting(wallet_balance);

        let sender_state = &mut ctx.accounts.sender_state;
        let recipient_state = &mut ctx.accounts.recipient_state;
        let sender_balance = ctx.accounts.sender.lamports();
        let recipient_balance = ctx.accounts.recipient.lamports();
        wallet_state.claim(
            recipient_state,
            wallet_key,
            &ctx.accounts.fund_wallet,
            &ctx.accounts.recipient,
            &ctx.accounts.system_program,
        )?;
        wallet_state.claim(
            sender_state,
            wallet_key,
            &ctx.accounts.fund_wallet,
            ctx.accounts.sender.as_ref(),
            &ctx.accounts.system_program,
        )?;
        sender_state.transfer(recipient_state, num_shares)?;
        msg!(
            "Sender claimed {} SOL",
            print_dec(ctx.accounts.sender.lamports() - sender_balance, 9)
        );
        msg!(
            "Receiver claimed {} SOL",
            print_dec(ctx.accounts.recipient.lamports() - recipient_balance, 9)
        );
        wallet_state.last_snapshot = ctx.accounts.fund_wallet.lamports()
            - wallet_state.dust
            - wallet_state.starting_balance;
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>) -> ProgramResult {
        let wallet_key = ctx.accounts.wallet_state.key();
        let wallet_balance = ctx.accounts.fund_wallet.lamports();
        let wallet_state = &mut ctx.accounts.wallet_state;
        let recipient_state = &mut ctx.accounts.recipient_state;
        assert_with_msg(
            wallet_balance >= wallet_state.last_snapshot,
            ProgramError::InvalidAccountData,
            "Wallet cannot have less SOL than last snaphshot",
        )?;
        wallet_state.update_internal_accounting(wallet_balance);
        let starting_balance = ctx.accounts.recipient.lamports();
        wallet_state.claim(
            recipient_state,
            wallet_key,
            &ctx.accounts.fund_wallet,
            &ctx.accounts.recipient,
            &ctx.accounts.system_program,
        )?;
        wallet_state.last_snapshot = ctx.accounts.fund_wallet.lamports()
            - wallet_state.dust
            - wallet_state.starting_balance;
        msg!(
            "Claimed {} SOL",
            print_dec(ctx.accounts.recipient.lamports() - starting_balance, 9)
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateFund<'info> {
    #[account(
        init,
        seeds = [
            creator.key().as_ref()
        ],
        bump,
        payer = creator,
        space = 89 + 8,
    )]
    wallet_state: Box<Account<'info, WalletState>>,
    #[account(
        mut,
        seeds = [
            wallet_state.key().as_ref(),
        ],
        bump,
    )]
    fund_wallet: AccountInfo<'info>,
    #[account(mut)]
    creator: Signer<'info>,
    #[account(
        init,
        seeds = [
            wallet_state.key().as_ref(),
            creator.key().as_ref()
        ],
        bump,
        payer = creator,
        space = 48 + 8,
    )]
    creator_state: Account<'info, Stake>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateStakeAccount<'info> {
    #[account(
        seeds = [
            wallet_state.creator.as_ref()
        ],
        bump,
    )]
    wallet_state: Box<Account<'info, WalletState>>,
    #[account(mut)]
    payer: Signer<'info>,
    user: AccountInfo<'info>,
    #[account(
        init,
        seeds = [
            wallet_state.key().as_ref(),
            user.key().as_ref()
        ],
        bump,
        payer = payer,
        space = 48 + 8,
    )]
    user_state: Account<'info, Stake>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TransferShares<'info> {
    #[account(
        mut,
        seeds = [
            wallet_state.creator.as_ref()
        ],
        bump,
    )]
    wallet_state: Box<Account<'info, WalletState>>,
    #[account(
        mut,
        seeds = [
            wallet_state.key().as_ref(),
        ],
        bump = wallet_state.fund_wallet_bump,
    )]
    fund_wallet: AccountInfo<'info>,
    #[account(mut)]
    sender: Signer<'info>,
    #[account(
        mut,
        seeds = [
            wallet_state.key().as_ref(),
            sender.key().as_ref()
        ],
        bump,
        constraint = sender_state.num_shares > 0, 
    )]
    sender_state: Account<'info, Stake>,
    #[account(
        mut,
        seeds = [
            wallet_state.key().as_ref(),
            recipient.key().as_ref()
        ],
        bump,
    )]
    recipient_state: Account<'info, Stake>,
    #[account(mut)]
    recipient: AccountInfo<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(
        mut,
        constraint = wallet_state.total_shares > 0,
        seeds = [
            wallet_state.creator.as_ref()
        ],
        bump,
    )]
    wallet_state: Box<Account<'info, WalletState>>,
    #[account(
        mut,
        seeds = [
            wallet_state.key().as_ref(),
        ],
        bump,
    )]
    fund_wallet: AccountInfo<'info>,
    #[account(mut)]
    recipient: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [
            wallet_state.key().as_ref(),
            recipient.key().as_ref()
        ],
        bump,
    )]
    recipient_state: Account<'info, Stake>,
    system_program: Program<'info, System>,
}
#[account]
pub struct Stake {
    pub fund_wallet: Pubkey,
    pub num_shares: u64,
    pub total_deposits_per_share_snapshot: u64,
}

impl Stake {
    pub fn initialize(&mut self, num_shares: u64, snaphshot: u64) {
        self.num_shares = num_shares;
        self.total_deposits_per_share_snapshot = snaphshot;
    }

    pub fn transfer(&mut self, other: &mut Account<Stake>, shares: u64) -> ProgramResult {
        other.num_shares = other
            .num_shares
            .checked_add(shares.min(self.num_shares))
            .ok_or(ProgramError::InvalidAccountData)?;
        self.num_shares = self.num_shares.saturating_sub(shares);
        Ok(())
    }
}

#[account]
pub struct WalletState {
    pub creator: Pubkey,
    pub dust: u64,
    pub total_shares: u64,
    pub total_deposits_per_share: u64,
    pub total_deposits: u128,
    pub last_snapshot: u64,
    pub starting_balance: u64,
    pub fund_wallet_bump: u8,
}

impl WalletState {
    pub fn initialize(
        &mut self,
        creator: Pubkey,
        total_shares: u64,
        starting_balance: u64,
        bump: u8,
    ) {
        self.creator = creator;
        self.dust = 0;
        self.total_shares = total_shares;
        self.total_deposits_per_share = 0;
        self.total_deposits = 0;
        self.last_snapshot = 0;
        self.starting_balance = starting_balance;
        self.fund_wallet_bump = bump;
    }

    pub fn update_internal_accounting(&mut self, wallet_balance: u64) {
        msg!(
            "Wallet Balance (minus rent): {}",
            print_dec(wallet_balance - self.starting_balance, 9)
        );
        let new_deposits =
            wallet_balance.saturating_sub(self.last_snapshot + self.dust + self.starting_balance);
        msg!("New Deposits: {}", print_dec(new_deposits, 9));
        msg!("Current Dust: {}", self.dust);
        if new_deposits > 0 {
            self.total_deposits += new_deposits as u128;
            self.dust += new_deposits % self.total_shares;
            let mut deposits_per_share = new_deposits / self.total_shares;
            if self.dust >= self.total_shares {
                deposits_per_share += 1;
                self.dust -= self.total_shares;
            }
            self.total_deposits_per_share += deposits_per_share;
        }
    }

    pub fn claim<'info>(
        &mut self,
        state: &mut Account<'info, Stake>,
        wallet_state_key: Pubkey,
        fund_wallet: &AccountInfo<'info>,
        recipient: &AccountInfo<'info>,
        system_program: &Program<'info, System>,
    ) -> ProgramResult {
        let withdrawable_amount = (self
            .total_deposits_per_share
            .saturating_sub(state.total_deposits_per_share_snapshot))
            * state.num_shares;

        if withdrawable_amount > 0 {
            invoke_signed(
                &system_instruction::transfer(
                    &fund_wallet.key(),
                    &recipient.key(),
                    withdrawable_amount,
                ),
                &[
                    fund_wallet.to_account_info(),
                    recipient.to_account_info(),
                    system_program.to_account_info(),
                ],
                &[&[wallet_state_key.as_ref(), &[self.fund_wallet_bump]]],
            )?;
        }
        state.total_deposits_per_share_snapshot = self.total_deposits_per_share;
        Ok(())
    }
}
