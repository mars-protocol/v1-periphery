use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, WasmMsg, StdResult, CosmosMsg, to_binary};
use cosmwasm_bignumber::{Decimal256, Uint256};



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub address_provider: Option<String>,
    pub maUST_token: Option<String>,
    pub init_timestamp: Option<u64>,
    pub min_duration: Option<u64>,
    pub max_duration: Option<u64>,
    pub denom: Option<String>,
    pub multiplier: Option<Decimal256>,
    pub lockdrop_incentives: Option<Uint256>
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
    UpdateConfig {
        new_config: InstantiateMsg,
    },    
    /// Callbacks; only callable by the contract itself.
    Callback(CallbackMsg),    
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    UpdateStateOnDeposit {
        user: Addr,
        duration: u64,
        ust_deposited: Uint256,
        mUST_prev_balance: Uint256
    },
    UpdateStateOnWithdraw {
        user: Addr,
        duration: u64,
        mUST_withdrawn: Uint256,
        prev_ust_balance: Uint256
    }
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn to_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
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
