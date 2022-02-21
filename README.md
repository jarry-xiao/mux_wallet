## MUX Wallet

This smart contract allows for users to share stake of a wallet (fund).

The following metadata is stored in order to ensure that all funds are fairly allocated:

```
// Per fund
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

// Per stakeholder
#[account]
pub struct Stake {
    pub num_shares: u64,
    pub total_deposits_per_share_snapshot: u64,
}
```

The high level idea is that at any given point in time, each stakeholder has the right to withdraw 
```
(fund.total_deposits_per_share - stakeholder.total_deposits_per_share_snapshot) * stakeholder.num_shares
```
As long as all funds are collected prior to any change in stake, every fund participant will have access to their fair share of the pool, and they can withdraw their allocated funds at any point in time
