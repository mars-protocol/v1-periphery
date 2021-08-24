use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr};
use cw_storage_plus::{Item, Map};

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

pub const STATE_KEY: &str = "state";
pub const STATE: Item<State> = Item::new(STATE_KEY);

pub const USER_INFO: Map<&Addr, UserInfo> = Map::new("users");
pub const LOCKUP_INFO: Map<&[u8], LockupInfo> = Map::new("lockup_position");


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub address_provider: Addr,                 // Address Provider Contract
    pub maUST_token: Addr,                      // maUST Token :: Minted upon UST deposits into red bank
    pub init_timestamp: u64,                    // Timestamp till when deposits can be made
    pub max_lock_duration: u64,                 // Max no. of days allowed for lockup
    pub min_lock_duration: u64,                 // Min. no. of days allowed for lockup
    pub weekly_multiplier: Decimal256,          // Reward multiplier for each extra day locked
    pub denom: String,                          // "uusd"
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub total_UST_locked: Uint256,
    pub total_maUST_locked: Uint256,
    pub global_reward_index: Decimal256,
    pub lockdrop_incentives: Uint256
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfo {
    pub total_ust_locked: Uint256,
    pub total_ma_UST_locked: Uint256,               // maUST locked
    pub lockup_positions: Vec<String>
}

impl Default for UserInfo {
    fn default() -> Self {
        UserInfo {
            total_ust_locked: Uint256::zero(),
            total_ma_UST_locked: Uint256::zero(),
            lockup_positions: vec![]
        }
    }
}



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockupInfo {
    pub user: Addr,                         // User Public Address
    pub duration: u64,                      // No. of days locked for 
    pub ma_UST_locked: Uint256,             // maUST locked
    pub reward_index: Decimal256,           // $MARS reward accrued over deposits
    pub pending_reward: Uint256,            // $MARS reward accrued
    pub lockdrop_reward: Uint256,           // $MARS rewarded for Lockdrop
    pub lockdrop_claimed: bool,
    pub unlock_timestamp: u64,              // Unlock Timestamp
}


impl Default for LockupInfo {
    fn default() -> Self {
        LockupInfo {
            user: Addr::unchecked("null"),
            duration: 0 as u64,
            ma_UST_locked: Uint256::zero(),
            reward_index: Decimal256::zero(),
            pending_reward: Uint256::zero(),
            lockdrop_reward: Uint256::zero(),
            lockdrop_claimed: false,
            unlock_timestamp: 0 as u64,
        }
    }
}



