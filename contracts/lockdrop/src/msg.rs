use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Addr, WasmMsg, StdResult, CosmosMsg, to_binary};
use cosmwasm_bignumber::{Decimal256, Uint256};
use crate::state::{State};



#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Account who can update config
    pub owner: String,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Option<String>,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: Option<String>,
    /// Timestamp till when deposits can be made
    pub init_timestamp: u64,
    /// Number of seconds for which lockup deposits will be accepted 
    pub deposit_window: u64,
    /// Number of seconds for which lockup withdrawals will be allowed 
    pub withdrawal_window: u64,
    /// Min. no. of days allowed for lockup 
    pub min_duration: u64,
    /// Max. no. of days allowed for lockup 
    pub max_duration: u64,
    /// "uusd" - Native token accepted by the contract for deposits
    pub denom: Option<String>,
    /// Lockdrop Reward multiplier 
    pub weekly_multiplier: Option<Decimal256>,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Option<Uint256>
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateConfigMsg {
    /// Account who can update config
    pub owner: Option<String>,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: Option<String>,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: Option<String>,
    /// Timestamp till when deposits can be made
    pub init_timestamp: Option<u64>,
    /// Number of seconds for which lockup deposits will be accepted 
    pub deposit_window: Option<u64>,
    /// Number of seconds for which lockup withdrawals will be allowed 
    pub withdrawal_window: Option<u64>,
    /// Min. no. of days allowed for lockup 
    pub min_duration: Option<u64>,
    /// Max. no. of days allowed for lockup 
    pub max_duration: Option<u64>,
    /// Lockdrop Reward multiplier 
    pub weekly_multiplier: Option<Decimal256>,
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
        new_config: UpdateConfigMsg,
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
    UpdateStateOnClaim {
        user: Addr,
        prev_xmars_balance: Uint256
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
    Config {},
    State {},
    UserInfo { address: String},
    LockUpInfo { address: String, duration: u64},
    LockUpInfoWithId { lockup_id: String},
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can update config
    pub owner: String,
    /// Contract used to query addresses related to red-bank (MARS Token)
    pub address_provider: String,
    ///  maUST token address - Minted upon UST deposits into red bank
    pub ma_ust_token: String,    
    /// Timestamp till when deposits can be made
    pub init_timestamp: u64,
    /// Min. no. of days allowed for lockup 
    pub min_duration: u64,
    /// Max. no. of days allowed for lockup 
    pub max_duration: u64,
    /// Lockdrop Reward multiplier 
    pub multiplier: Decimal256,
    /// Total MARS lockdrop incentives to be distributed among the users
    pub lockdrop_incentives: Uint256
}


pub type GlobalStateResponse = State;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub total_ust_locked: Uint256,
    pub total_maust_share: Uint256,
    pub lockup_position_ids: Vec<String>
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LockUpInfoResponse {
        /// Lockup Duration
        pub duration: u64,
        /// UST locked as part of this lockup position
        pub ust_locked: Uint256,            
        /// MA-UST share
        pub maust_balance: Uint256,            
        /// Lockdrop incentive distributed to this position
        pub lockdrop_reward: Uint256,         
        /// Boolean value indicating if the lockdrop_reward has been claimed or not
        pub lockdrop_claimed: bool,
        /// Value used to calculate deposit_rewards accured by this position
        pub reward_index: Decimal256, 
        /// Pending rewards to be claimed by the user        
        pub pending_reward: Uint256,            
        /// Timestamp beyond which this position can be unlocked
        pub unlock_timestamp: u64,   
}

