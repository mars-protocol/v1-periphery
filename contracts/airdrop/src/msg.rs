use cosmwasm_std::{Addr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub mars_address: String,
    pub admin: String,
    pub terra_merkle_roots: Vec<String>,
    pub evm_merkle_roots: Vec<String>,    
    pub till_timestamp: u64
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateTerraMerkleRoots {
        merkle_roots: Vec<String>,
    },
    UpdateEvmMerkleRoots {
        merkle_roots: Vec<String>,
    },
    UpdateClaimDuration {
        new_timestamp: u64,
    },
    TerraClaim {
        amount: Uint128,
        merkle_proof: Vec<String>,
        root_index: u32
    },
    EvmClaim {
        eth_address: String,
        claim_amount: Uint128,
        signature: String,
        merkle_proof: Vec<String>,
        root_index: u32
    },
    TransferMarsTokens {
        recepient: String,
        amount: Uint128,
    },
    UpdateAdmin {
        new_admin: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    IsClaimed {
        address: String,
     },
     IsValidSignature {
        user_address: String,
        eth_signature: String,
        signed_msg: String,                
     },
}

pub type ConfigResponse = InstantiateMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ClaimResponse {
    pub is_claimed: bool,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SignatureResponse {
    pub is_valid: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    Cw20 { contract_addr: Addr },
    Native { denom: String },
}
