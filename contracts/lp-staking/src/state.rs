use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Addr,StdError, StdResult};
use cw_storage_plus::{Item, Map};



pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

pub const STATE_KEY: &str = "state";
pub const STATE: Item<State> = Item::new(STATE_KEY);

pub const STAKER_KEY: &str = "staker";
pub const STAKER_INFO: Map<&Addr, StakerInfo> = Map::new(STAKER_KEY);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub mars_token: Addr,
    pub staking_token: Addr,
    pub init_timestamp: u64,                // Block no. from when rewards will be distributed
    pub till_timestamp: u64,                  // Block no. till when rewards will be distributed
    pub cycle_duration: u64,             // Blocks per cycle
    pub reward_increase: Decimal256,     // % increase in rewards per cycle
}

impl Config {
    pub fn validate(&self) -> StdResult<()> { 
        if (&self.init_timestamp < &self.till_timestamp) && (&self.reward_increase < &Decimal256::one()) && (&self.cycle_duration > &100) {
            return Ok(());
        }
        return Err(StdError::generic_err("Invalid configuration"));
    }
}






#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub cycle_init_timestamp: u64,             // Current Cycle
    pub cycle_rewards: Uint256,          // $MARS distributed during the cycle 
    pub last_distributed: u64,
    pub total_bond_amount: Uint256,
    pub global_reward_index: Decimal256,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub reward_index: Decimal256,
    pub bond_amount: Uint256,
    pub pending_reward: Uint256,
}

impl Default for StakerInfo {
    fn default() -> Self {
        StakerInfo {
            reward_index: Decimal256::one(),
            bond_amount: Uint256::zero(),
            pending_reward: Uint256::zero(),
        }
    }
}
