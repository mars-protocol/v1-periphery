use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, StdResult, Uint128, WasmMsg};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub mars_token_address: String,
    pub astro_token_address: String,
    pub airdrop_contract_address: String,
    pub lockdrop_contract_address: String,
    pub generator_contract: String,
    pub mars_vesting_duration: u64,
    pub lp_tokens_vesting_duration: u64,
    pub init_timestamp: u64,
    pub mars_deposit_window: u64,
    pub ust_deposit_window: u64,
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateConfigMsg {
    pub owner: Option<String>,
    pub astroport_lp_pool: Option<String>,
    pub mars_lp_staking_contract: Option<String>,
    pub generator_contract: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        new_config: UpdateConfigMsg,
    },

    DepositUst {},
    WithdrawUst {
        amount: Uint128,
    },

    AddLiquidityToAstroportPool {
        slippage: Option<Decimal>,
    },
    StakeLpTokens {
        single_incentive_staking: bool,
        dual_incentives_staking: bool,
    },

    ClaimRewards {
        withdraw_unlocked_shares: bool,
    },
    Callback(CallbackMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    DepositMarsTokens { user_address: Addr },
    IncreaseMarsIncentives {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    UpdateStateOnRewardClaim {
        user_address: Option<Addr>,
        prev_mars_balance: Uint128,
        prev_astro_balance: Uint128,
        withdraw_lp_shares: Uint128,
    },
    UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: Uint128,
    },
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
    UserInfo { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub mars_token_address: String,
    pub astro_token_address: String,
    pub airdrop_contract_address: String,
    pub lockdrop_contract_address: String,
    pub astroport_lp_pool: Option<Addr>,
    pub lp_token_address: Option<Addr>,
    pub mars_lp_staking_contract: Option<Addr>,
    pub generator_contract: String,
    pub mars_rewards: Uint128,
    pub mars_vesting_duration: u64,
    pub lp_tokens_vesting_duration: u64,
    pub init_timestamp: u64,
    pub mars_deposit_window: u64,
    pub ust_deposit_window: u64,
    pub withdrawal_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub total_mars_deposited: Uint128,
    pub total_ust_deposited: Uint128,
    pub lp_shares_minted: Uint128,
    pub lp_shares_withdrawn: Uint128,
    pub are_staked_for_single_incentives: bool,
    pub are_staked_for_dual_incentives: bool,
    pub pool_init_timestamp: u64,
    pub global_mars_reward_index: Decimal,
    pub global_astro_reward_index: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserInfoResponse {
    pub mars_deposited: Uint128,
    pub ust_deposited: Uint128,
    pub ust_withdrawn_flag: bool,
    pub lp_shares: Uint128,
    pub withdrawn_lp_shares: Uint128,
    pub withdrawable_lp_shares: Uint128,
    pub total_auction_incentives: Uint128,
    pub withdrawn_auction_incentives: Uint128,
    pub withdrawable_auction_incentives: Uint128,
    pub mars_reward_index: Decimal,
    pub withdrawable_mars_incentives: Uint128,
    pub withdrawn_mars_incentives: Uint128,
    pub astro_reward_index: Decimal,
    pub withdrawable_astro_incentives: Uint128,
    pub withdrawn_astro_incentives: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

