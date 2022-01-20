use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USERS: Map<&Addr, UserInfo> = Map::new("users");

//----------------------------------------------------------------------------------------
// Storage types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// Account who can update config
    pub owner: Addr,
    ///  MARS token address
    pub mars_token_address: Addr,
    ///  ASTRO token address
    pub astro_token_address: Addr,
    /// Airdrop Contract address
    pub airdrop_contract_address: Addr,
    /// Lockdrop Contract address
    pub lockdrop_contract_address: Addr,
    ///  MARS-UST LP Pool address
    pub astroport_lp_pool: Option<Addr>,
    ///  MARS-UST LP Token address
    pub lp_token_address: Option<Addr>,
    ///  MARS LP Staking contract with which MARS-UST LP Tokens can be staked
    pub mars_lp_staking_contract: Option<Addr>,
    ///  Astroport Generator contract with which MARS-UST LP Tokens can be staked
    pub generator_contract: Addr,
    /// Total MARS token rewards to be used to incentivize boostrap auction participants
    pub mars_rewards: Uint128,
    /// Number of seconds over which MARS incentives are vested
    pub mars_vesting_duration: u64,
    ///  Number of seconds over which LP Tokens are vested
    pub lp_tokens_vesting_duration: u64,
    /// Timestamp since which MARS / UST deposits will be allowed
    pub init_timestamp: u64,
    /// Number of seconds post init_timestamp during which UST deposits / withdrawals will be allowed
    pub ust_deposit_window: u64,
    /// Number of seconds post init_timestamp during which MARS delegations (via lockdrop / airdrop) will be allowed
    pub mars_deposit_window: u64,
    /// Number of seconds post ust_deposit_window completion during which only partial UST withdrawals are allowed
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct State {
    /// Total MARS tokens delegated to the contract by lockdrop participants / airdrop recipients
    pub total_mars_deposited: Uint128,
    /// Total UST deposited in the contract
    pub total_ust_deposited: Uint128,
    /// Total LP shares minted post liquidity addition to the MARS-UST Pool
    pub lp_shares_minted: Uint128,
    /// Number of LP shares that have been withdrawn as they unvest
    pub lp_shares_withdrawn: Uint128,
    /// True if MARS--UST LP Shares are currently staked with the MARS LP Staking contract
    pub are_staked_for_single_incentives: bool,
    /// True if MARS--UST LP Shares are currently staked with Astroport Generator for dual staking incentives
    pub are_staked_for_dual_incentives: bool,
    /// Timestamp at which liquidity was added to the MARS-UST LP Pool
    pub pool_init_timestamp: u64,
    /// index used to keep track of $MARS claimed as LP staking rewards and distribute them proportionally among the auction participants
    pub global_mars_reward_index: Decimal,
    /// index used to keep track of $ASTRO claimed as LP staking rewards and distribute them proportionally among the auction participants
    pub global_astro_reward_index: Decimal,
}

impl Default for State {
    fn default() -> Self {
        State {
            total_mars_deposited: Uint128::zero(),
            total_ust_deposited: Uint128::zero(),
            lp_shares_minted: Uint128::zero(),
            lp_shares_withdrawn: Uint128::zero(),
            pool_init_timestamp: 0u64,
            are_staked_for_single_incentives: false,
            are_staked_for_dual_incentives: false,
            global_mars_reward_index: Decimal::zero(),
            global_astro_reward_index: Decimal::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    // Total MARS Tokens delegated by the user
    pub mars_deposited: Uint128,
    // Total UST deposited by the user
    pub ust_deposited: Uint128,
    // Withdrawal counter to capture if the user already withdrew UST during the "only withdrawals" window
    pub ust_withdrawn_flag: bool,
    // User's LP share balance
    pub lp_shares: Uint128,
    // LP shares withdrawn by the user
    pub withdrawn_lp_shares: Uint128,
    // User's MARS rewards for participating in the auction
    pub total_auction_incentives: Uint128,
    // MARS rewards withdrawn by the user
    pub withdrawn_auction_incentives: Uint128,
    // MARS staking incentives (LP token staking) withdrawn by the user
    pub withdrawn_mars_incentives: Uint128,
    // ASTRO staking incentives (LP token staking) withdrawn by the user
    pub withdrawn_astro_incentives: Uint128,
    // Index used to calculate user's $MARS staking rewards
    pub mars_reward_index: Decimal,
    // Index used to calculate user's $ASTRO staking rewards
    pub astro_reward_index: Decimal,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            mars_deposited: Uint128::zero(),
            ust_deposited: Uint128::zero(),
            ust_withdrawn_flag: false,
            lp_shares: Uint128::zero(),
            withdrawn_lp_shares: Uint128::zero(),
            total_auction_incentives: Uint128::zero(),
            withdrawn_auction_incentives: Uint128::zero(),
            withdrawn_mars_incentives: Uint128::zero(),
            withdrawn_astro_incentives: Uint128::zero(),
            mars_reward_index: Decimal::zero(),
            astro_reward_index: Decimal::zero(),
        }
    }
}
