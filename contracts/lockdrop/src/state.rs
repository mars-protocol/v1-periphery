use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use cosmwasm_bignumber::{Decimal256, Uint256};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

pub const STATE_KEY: &str = "state";
pub const STATE: Item<State> = Item::new(STATE_KEY);

pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("users");
pub const LOCKUP_INFO: Map<&[u8], LockupInfo> = Map::new("lockup_position");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Account who can update config
    pub owner: Addr,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Addr,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: Addr,
    /// Timestamp when Contract will start accepting deposits
    pub init_timestamp: u64,
    /// Deposit Window Length
    pub deposit_window: u64,
    /// Withdrawal Window Length
    pub withdrawal_window: u64,
    /// Min. no. of weeks allowed for lockup
    pub min_lock_duration: u64,
    /// Max. no. of weeks allowed for lockup
    pub max_lock_duration: u64,
    /// Number of seconds per week
    pub seconds_per_week: u64,
    /// Lockdrop Reward multiplier
    pub weekly_multiplier: Decimal256,
    /// "uusd" - Native token accepted by the contract for deposits
    pub denom: String,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total UST deposited at the end of Lockdrop window. This value remains unchanged post the lockdrop window
    pub final_ust_locked: Uint256,
    /// maUST minted at the end of Lockdrop window upon UST deposit in red bank. This value remains unchanged post the lockdrop window
    pub final_maust_locked: Uint256,
    /// UST deposited in the contract. This value is updated real-time upon each UST deposit / unlock
    pub total_ust_locked: Uint256,
    /// maUST held by the contract. This value is updated real-time upon each maUST withdrawal from red bank
    pub total_maust_locked: Uint256,
    /// Total weighted deposits
    pub total_deposits_weight: Uint256,
    /// Ratio of MARS rewards accured to total_maust_locked. Used to calculate MARS incentives accured by each user
    pub global_reward_index: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    /// Total UST amount deposited by the user across all his lockup positions
    pub total_ust_locked: Uint256,
    /// Contains lockup Ids of the User's lockup positions with different durations / deposit amounts
    pub lockup_positions: Vec<String>,
    /// Boolean value indicating if the lockdrop_rewards for the lockup positions have been claimed or not
    pub lockdrop_claimed: bool,
    /// Value used to calculate deposit_rewards (XMARS) accured by the user
    pub reward_index: Decimal256,
    /// Pending rewards to be claimed by the user        
    pub pending_xmars: Uint256,
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            total_ust_locked: Uint256::zero(),
            lockup_positions: vec![],
            lockdrop_claimed: false,
            reward_index: Decimal256::zero(),
            pending_xmars: Uint256::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockupInfo {
    /// Lockup Duration
    pub duration: u64,
    /// UST locked as part of this lockup position
    pub ust_locked: Uint256,
    /// Lockdrop incentive distributed to this position
    pub lockdrop_reward: Uint256,
    /// Timestamp beyond which this position can be unlocked
    pub unlock_timestamp: u64,
}

impl Default for LockupInfo {
    fn default() -> Self {
        LockupInfo {
            duration: 0 as u64,
            ust_locked: Uint256::zero(),
            lockdrop_reward: Uint256::zero(),
            unlock_timestamp: 0 as u64,
        }
    }
}
