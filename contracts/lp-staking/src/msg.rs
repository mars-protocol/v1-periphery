use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{Decimal, Uint128, Addr};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub mars_token: String,
    pub staking_token: String,             // LP token of MARS-UST pair contract
    pub init_timestamp: u64,                // Block no. from when rewards will be distributed
    pub till_timestamp: u64,                  // Block no. till when rewards will be distributed
    pub cycle_rewards: Uint256,          // $MARS distributed during the 1st cycle 
    pub cycle_duration: u64,             // Blocks per cycle
    pub reward_increase: Decimal256,     // % increase in rewards per cycle
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Unbond {
        amount: Uint256,
    },
    /// Claim pending rewards
    Claim {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Bond {},
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {
        timestamp: Option<u64>,
    },
    StakerInfo {
        staker: String,
        timestamp: Option<u64>,
    },
    Timestamp {}
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub mars_token: String,
    pub staking_token: String,
    pub init_timestamp: u64,              
    pub till_timestamp: u64,                
    pub cycle_duration: u64,  
    pub reward_increase: Decimal256,         
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub cycle_init_timestamp: u64,
    pub cycle_rewards: Uint256,          
    pub last_distributed: u64,
    pub total_bond_amount: Uint256,
    pub global_reward_index: Decimal256,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfoResponse {
    pub staker: String,
    pub reward_index: Decimal256,
    pub bond_amount: Uint256,
    pub pending_reward: Uint256,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TimeResponse {
    pub timestamp: u64,
}


