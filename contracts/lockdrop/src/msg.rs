use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, WasmMsg, StdResult, CosmosMsg, to_binary};
use cosmwasm_bignumber::{Decimal256, Uint256};



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Account who can update config
    pub owner: Option<String>,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Option<String>,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: Option<String>,
    /// Timestamp till when deposits can be made
    pub init_timestamp: Option<u64>,
    /// Min. no. of days allowed for lockup 
    pub min_duration: Option<u64>,
    /// Max. no. of days allowed for lockup 
    pub max_duration: Option<u64>,
    /// "uusd" - Native token accepted by the contract for deposits
    pub denom: Option<String>,
    /// Lockdrop Reward multiplier 
    pub multiplier: Option<Decimal256>,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint256>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    DepositUST {  
        duration: u64 
    },
    WithdrawUST {
        duration: u64,
        amount: Uint256
    },
    Unlock {  
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
    UpdateStateOnRedBankDeposit {
        prev_ma_ust_balance: Uint256
    },
    UpdateStateOnWithdraw {
        user: Addr,
        duration: u64,
        m_ust_withdrawn: Uint256,
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
