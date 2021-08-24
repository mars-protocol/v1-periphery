use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, Decimal, Uint128};
use cosmwasm_bignumber::{Decimal256, Uint256};



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub red_bank: String,
    pub mars_token: String,
    pub maUST_token: String,
    pub incentives_contract: String,
    pub init_timestamp: u64,
    pub min_duration: u64,
    pub max_duration: u64,
    pub borrow_ltv: Decimal256,
    pub denom: String,
    pub multiplier: Decimal256,
    pub owner: String
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    LockUST {  
        duration: u64 
    },
    UnlockUST {  
        duration: u64 
    },
    ClaimRewards {
    },
    BorrowUST {
        amount: Uint256
    },
    RepayUST {
    }    
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    GetCount {},
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CountResponse {
    pub count: i32,
}
