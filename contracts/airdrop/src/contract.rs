use cosmwasm_std::{ 
    attr, entry_point, Binary, Deps, DepsMut, MessageInfo, Env, Response, 
    StdError, StdResult, Uint128, WasmMsg, to_binary
};
use std::convert::{TryInto, TryFrom};
use std::cmp::Ordering;

use cw20_base::msg::{ExecuteMsg as CW20ExecuteMsg };
use crate::utils::{ normalize_recovery_id, hash_message, get_public_key_from_verify_key };
use crate::msg::{ClaimResponse, ConfigResponse,SignatureResponse, ExecuteMsg, InstantiateMsg, QueryMsg  } ;
use crate::state::{Config, CONFIG, CLAIMEES};

use hex;
use k256::ecdsa::recoverable::{Id as RecoverableId, Signature as RecoverableSignature};
use k256::ecdsa::Signature;
use sha3::{ Digest, Keccak256 };


//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------


#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {

    if msg.till_timestamp <= _env.block.time.seconds() {
        return Err(StdError::generic_err("Invalid timestamp provided"));
    }

    let config = Config {
        mars_token_address: deps.api.addr_validate(&msg.mars_token_address)?,
        owner: deps.api.addr_validate(&msg.owner)?,
        terra_merkle_roots: msg.terra_merkle_roots,
        evm_merkle_roots: msg.evm_merkle_roots,
        till_timestamp: msg.till_timestamp,
    };

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::default())
}


#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
)  -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::UpdateTerraMerkleRoots { 
            merkle_roots
        }  => update_terra_merkle_roots(deps, env, info, merkle_roots),
        ExecuteMsg::UpdateEvmMerkleRoots { 
            merkle_roots 
        }  => update_evm_merkle_roots(deps, env, info,  merkle_roots),
        ExecuteMsg::Updateowner { 
            new_owner 
        }  => handle_update_owner(deps, env, info,  new_owner),
        ExecuteMsg::UpdateClaimDuration { 
            new_timestamp 
        }  => handle_update_claim_duration(deps, env, info,  new_timestamp),
        ExecuteMsg::TerraClaim { 
            amount, 
            merkle_proof, 
            root_index
        } => handle_terra_user_claim(deps, env, info,  amount, merkle_proof, root_index),
        ExecuteMsg::EvmClaim { 
            eth_address, 
            claim_amount, 
            signature, 
            merkle_proof, 
            root_index
        } => handle_evm_user_claim(deps, env, info,  eth_address, claim_amount, signature, merkle_proof, root_index),       
        ExecuteMsg::TransferMarsTokens { 
            recepient, 
            amount 
        }  => handle_transfer_mars(deps, env,  info, recepient, amount),
    }
}


pub fn query(deps: Deps, msg: QueryMsg,) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::IsClaimed { 
            address 
        } => to_binary(&check_terra_user_claimed(deps, address)?),
        QueryMsg::IsValidSignature { 
            user_address, 
            eth_signature, 
            signed_msg 
        } => to_binary(&verify_signature(deps, user_address, eth_signature, signed_msg )?),
    }
}


//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------


/// @dev Executes an airdrop claim by a Terra Address
/// @param amount : Airdrop to be claimed by the account
/// @param merkle_proof : Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param root_index : Merkle Tree root identifier to be used for verification
pub fn handle_terra_user_claim( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    claim_amount: Uint128,  
    merkle_proof: Vec<String>, 
    root_index: u32
) -> Result<Response, StdError> {

    let config = CONFIG.load(deps.storage)?;
    let user_account =  info.sender;

    // CHECK IF AIRDROP CLIAIM WINDOW IS OPEN
    if config.till_timestamp <= _env.block.time.seconds() {
        return Err(StdError::generic_err("Airdrop Claim period has concluded"));
    }

    // let terra_adr_50_percent = user_account.clone().to_string() + &"_50Percent".to_string();

    // CLAIM : CHECK IF CLAIMED
    let res = CLAIMEES.load(deps.storage, &user_account.as_bytes() )?;
    if res {
            return Err(StdError::generic_err("Account has already claimed the Airdrop"));
        }
  
    // MERKLE PROOF VERIFICATION
    if !verify_claim(user_account.to_string() , claim_amount, merkle_proof.clone(), config.terra_merkle_roots[root_index as usize].clone()) {
        return Err(StdError::generic_err("Incorrect Merkle Proof"));
    }

    // CLAIM : MARK CLAIMED
    CLAIMEES.save(deps.storage, &user_account.as_bytes(), &true )?;

    // AIRDROP: CLAIM AMOUNT TRANSFERRED
    let message_ = WasmMsg::Execute {
        contract_addr: config.mars_token_address.to_string(),
        funds: vec![],
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: user_account.to_string(),
            amount: claim_amount.into(),
        })?,
    };

    Ok(Response::new()        
    .add_message(message_)    
    .add_attributes(vec![
        attr("action", "claim_for_terra"),
        attr("claimed", user_account.to_string() ),
        attr("amount", claim_amount)
    ]))
}


/// @dev Executes an airdrop claim by an EVM Address
/// @param eth_address : EVM address claiming the airdop
/// @param claim_amount : Airdrop amount claimed by the user
/// @param signature : ECDSA Signature string generated by singing the message = ethereum address (lower-case without '0x' prefix) + recepient address + claim amount
/// @param merkle_proof : Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param root_index : Merkle Tree root identifier to be used for verification
pub fn handle_evm_user_claim( 
    deps: DepsMut,
    _env: Env, 
    info: MessageInfo,
    eth_address: String, 
    claim_amount: Uint128, 
    signature: String, 
    merkle_proof: Vec<String>, 
    root_index: u32
) -> Result<Response, StdError> {

    let config = CONFIG.load(deps.storage)?;
    let recepient_account =  info.sender;

    // CHECK IF AIRDROP CLIAIM WINDOW IS OPEN
    if config.till_timestamp <= _env.block.time.seconds() {
        return Err(StdError::generic_err("Airdrop Claim period has concluded"));
    }

    // CLAIM : CHECK IF CLAIMED
    let res = CLAIMEES.load(deps.storage, eth_address.clone().as_bytes() )?;// read_claimed(&deps.storage).may_load( eth_address.clone().as_bytes() )?;
    if res {
        return Err(StdError::generic_err("Account has already claimed the Airdrop"));
    }

    // MERKLE PROOF VERIFICATION
    if !verify_claim(eth_address.clone() , claim_amount, merkle_proof.clone(), config.evm_merkle_roots[root_index as usize].clone()) {
        return Err(StdError::generic_err("Incorrect Merkle Proof"));
    }

    // SIGNATURE VERIFICATION
    let signed_msg = eth_address.clone() + &recepient_account.to_string().clone() + &claim_amount.to_string();
    if !handle_verify_signature(eth_address.clone() , signature.clone(),  signed_msg.clone()) {
        return Err(StdError::generic_err("Invalid Signature"));
    }

    // CLAIM : MARK CLAIMED
    CLAIMEES.save(deps.storage, eth_address.clone().as_bytes(), &true )?;

    // AIRDROP: CLAIM AMOUNT TRANSFERRED
    let message_ = WasmMsg::Execute {
        contract_addr: config.mars_token_address.to_string(),
        funds: vec![],
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: recepient_account.to_string(),
            amount: claim_amount.into(),
        })?,
    };

    Ok(Response::new()        
    .add_message(message_)    
    .add_attributes(vec![
        attr("action", "claim_for_evm"),
        attr("claimed", eth_address.to_string() ),
        attr("recepient", recepient_account.to_string() ),
        attr("amount", claim_amount),
    ]))

}




/// @dev Updates Merkle Tree roots to be used for Claim Verification by Terra users
/// @param merkle_roots :Merkle Tree roots to be used for Claim Verification by Terra users
pub fn update_terra_merkle_roots( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    merkle_roots: Vec<String> 
) -> Result<Response, StdError> {

    let mut config = CONFIG.load(deps.storage)?;

    // owner RESTRICTION CHECK
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not owner!"));        
    }

    config.terra_merkle_roots = merkle_roots;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()        
    .add_attributes(vec![
        attr("action", "TerraMerkleRootsUpdated"),
    ]))

}


/// @dev Updates Merkle Tree roots to be used for Claim Verification by EVM users
/// @param merkle_roots :Merkle Tree roots to be used for Claim Verification by EVM users
pub fn update_evm_merkle_roots( 
    deps: DepsMut, 
    _env: Env,  
    info: MessageInfo,
    merkle_roots: Vec<String> 
) -> Result<Response, StdError> {

    let mut config = CONFIG.load(deps.storage)?;

    // owner RESTRICTION CHECK
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not owner!"));        
    }

    config.evm_merkle_roots = merkle_roots;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()        
    .add_attributes(vec![
        attr("action", "EvmMerkleRootsUpdated"),
    ]))
}


/// @dev Updates the owner
/// @param new_owner New owner address
pub fn handle_update_owner( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    new_owner: String 
) -> Result<Response, StdError> {

    let mut config = CONFIG.load(deps.storage)?; 

    // owner RESTRICTION CHECK
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not owner!"));        
    }
    
    config.owner = deps.api.addr_validate(&new_owner)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()        
    .add_attributes(vec![
        attr("action", "Updateowner"),
        attr("new_owner", new_owner.to_string() ),
    ]))
}


/// @dev Updates the Claim duration ( Timestamp )
/// @param new_timestamp New timestamp till which the Airdrop can be claimed
pub fn handle_update_claim_duration( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    new_timestamp: u64 
) -> Result<Response, StdError> {
    
    let mut config = CONFIG.load(deps.storage)?; 

    // owner RESTRICTION CHECK
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not owner!"));        
    }

    if new_timestamp <= config.till_timestamp {
        return Err(StdError::generic_err("Claim duration can only be extended. Invalid timestamp provided"));
    }

    config.till_timestamp = new_timestamp;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()        
    .add_attributes(vec![
        attr("action", "UpdateClaimDuration"),
        attr("till_timestamp", new_timestamp.to_string() ),
    ]))

}


 
/// @dev Transfer MARS Tokens to the recepient address
/// @param recepient Recepient receiving the MARS tokens
/// @param amount Amount of MARS to be transferred
pub fn handle_transfer_mars( 
    deps: DepsMut, 
    _env: Env, 
    info: MessageInfo,
    recepient: String, 
    amount: Uint128
) -> Result<Response, StdError> {

    let config = CONFIG.load(deps.storage)?;

    // owner RESTRICTION CHECK
    if info.sender != config.owner {
        return Err(StdError::generic_err("Sender not authorized!"));        
    }

    let message_ = WasmMsg::Execute {
        contract_addr: config.mars_token_address.to_string(),
        funds: vec![],
        msg: to_binary(&CW20ExecuteMsg::Transfer {
            recipient: recepient.clone(),
            amount: amount.into(),
        })?,
    };

    Ok(Response::new()
    .add_message(message_)        
    .add_attributes(vec![
        attr("action", "TransferMarsTokens"),
        attr("recepient", recepient.to_string() ),
        attr("amount", amount ),
    ]))

}





//----------------------------------------------------------------------------------------
// Query functions
//----------------------------------------------------------------------------------------


/// @dev Returns the airdrop configuration
fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse { 
        mars_token_address: config.mars_token_address.to_string(),
        owner: config.owner.to_string(),
        terra_merkle_roots: config.terra_merkle_roots, 
        evm_merkle_roots: config.evm_merkle_roots, 
        till_timestamp: config.till_timestamp
    })
}

/// @dev Returns true if the user has claimed the airdrop [EVM addresses to be provided in lower-case without the '0x' prefix]
fn check_terra_user_claimed(deps: Deps, address: String  ) -> StdResult<ClaimResponse> {

    let res = CLAIMEES.load(deps.storage, address.clone().as_bytes() )?;
    Ok(ClaimResponse {  is_claimed: res }) 
}

/// @dev Returns true if the ECDSA signature string generated by signing the 'msg' with the ethereum wallet is valid. [EVM addresses to be provided in lower-case without the '0x' prefix]
fn verify_signature(_deps: Deps, user_address: String, eth_signature: String, signed_msg: String  ) -> StdResult<SignatureResponse> {
    let mut is_valid = false;
    if handle_verify_signature(user_address, eth_signature, signed_msg)  {
        is_valid = true;
    }
    Ok(SignatureResponse {
        is_valid: is_valid
    })
}

//----------------------------------------------------------------------------------------
// Helper functions
//----------------------------------------------------------------------------------------

/// @dev Verify whether a claim is valid
/// @param account Account on behalf of which the airdrop is to be claimed (etherum addresses without `0x` prefix)
/// @param amount Airdrop amount to be claimed by the user
/// @param merkle_proof Array of hashes to prove the input is a leaf of the Merkle Tree
/// @param merkle_root Hash of Merkle tree's root
fn verify_claim( account: String, amount: Uint128, merkle_proof: Vec<String>, merkle_root: String) -> bool {

    let leaf = account.clone() + &amount.to_string();
    let mut hash_buf  = Keccak256::digest(leaf.as_bytes()).as_slice().try_into().expect("Wrong length");
    let mut hash_str: String; 

    for p in merkle_proof {
        let mut proof_buf: [u8; 32] = [0; 32];
        hex::decode_to_slice(p, &mut proof_buf).unwrap();
        let proof_buf_str = hex::encode(proof_buf);
        hash_str = hex::encode(hash_buf);

        if proof_buf_str.cmp(&hash_str.clone()) == Ordering::Greater {
            hash_buf = Keccak256::digest(&[hash_buf, proof_buf].concat()).as_slice().try_into().expect("Wrong length")
        } else {
            hash_buf = Keccak256::digest(&[proof_buf, hash_buf].concat()).as_slice().try_into().expect("Wrong length")
        }
    }
        
    hash_str = hex::encode(hash_buf);
    return merkle_root == hash_str;
}


/// @dev Verify whether Signature provided is valid
/// @param address Ethereum account (without '0x' prefix) on behalf of which the airdrop is to be claimed
/// @param eth_signature Encoded hexadecimal string of the ECDSA signature of the signed message
/// @param msg Message signed by the user 
pub fn handle_verify_signature( address: String, eth_signature: String, msg: String ) -> bool {

    let mut test_sig = vec![0; 65];
    test_sig = hex::decode(eth_signature.clone()).unwrap(); 

    let signature = Signature::try_from( &test_sig[0..64] ).unwrap(); 
    let id = RecoverableId::new( normalize_recovery_id(test_sig[64]) ).unwrap();
    let recoverable_signature = RecoverableSignature::new(&signature, id).unwrap(); 
    
    let message_hash = hash_message(msg.into_bytes());

    let verify_key = recoverable_signature.recover_verify_key_from_digest_bytes( message_hash.as_ref().into() ).unwrap(); //.or_else(|_| return false);
    let public_key = get_public_key_from_verify_key(&verify_key );

    return public_key == address;
}




#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        testing::{mock_env, mock_info, mock_dependencies, MOCK_CONTRACT_ADDR},
        Addr, BankMsg, CosmosMsg, OwnedDeps, Timestamp,BlockInfo, ContractInfo
    };
    // use cosmwasm_std::testing::{mock_env, mock_info, mock_dependencies, MockApi, MockStorage, MOCK_CONTRACT_ADDR};
    use crate::state::{Config, CONFIG, CLAIMEES};
    use cw20_base::msg::{ExecuteMsg as CW20ExecuteMsg };
    use crate::msg::{ClaimResponse, ConfigResponse,SignatureResponse, ExecuteMsg, InstantiateMsg, QueryMsg  } ;
    use cosmwasm_std::{coin, from_binary};


    pub struct MockEnvParams {
        pub block_time: Timestamp,
        pub block_height: u64,
    }
    
    impl Default for MockEnvParams {
        fn default() -> Self {
            MockEnvParams {
                block_time: Timestamp::from_nanos(1_571_797_419_879_305_533),
                block_height: 1,
            }
        }
    }
    
    /// mock_env replacement for cosmwasm_std::testing::mock_env
    pub fn mock_env_custom(mock_env_params: MockEnvParams) -> Env {
        Env {
            block: BlockInfo {
                height: mock_env_params.block_height,
                time: mock_env_params.block_time,
                chain_id: "cosmos-testnet-14002".to_string(),
            },
            contract: ContractInfo {
                address: Addr::unchecked(MOCK_CONTRACT_ADDR),
            },
        }
    }

    /// quick mock info with just the sender
    // TODO: Maybe this one does not make sense given there's a very smilar helper in cosmwasm_std
    // pub fn mock_info_custom(sender: &str) -> MessageInfo {
    //     MessageInfo {
    //         sender: Addr::unchecked(sender),
    //         funds: vec![],
    //     }
    // }


    #[test]
    fn test_proper_initialization() {
        let mut deps = mock_dependencies(&[]);

        let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
        let evm_merkle_roots = vec![ "evm_merkle_roots".to_string() ];
        let init_timestamp = 1_000_000_000;

        let msg = InstantiateMsg {
            mars_token_address: "mars_token_contract".to_string(),
            owner: "owner_address".to_string(),
            till_timestamp: init_timestamp + 1000,
            terra_merkle_roots: terra_merkle_roots.clone(),
            evm_merkle_roots: evm_merkle_roots.clone() 
        };

        let info = mock_info("creator", &[]);
        let env = mock_env_custom(MockEnvParams {
            block_time: Timestamp::from_seconds(init_timestamp),
            ..Default::default()
        });

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), env,info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), QueryMsg::Config {}).unwrap();
        let value: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!("mars_token_contract".to_string(), value.mars_token_address);
        assert_eq!("owner_address".to_string(), value.owner);
        assert_eq!(terra_merkle_roots.clone(), value.terra_merkle_roots);
        assert_eq!(evm_merkle_roots.clone(), value.evm_merkle_roots);
    }

    #[test]
    fn test_update_owner() {

        let mut deps = mock_dependencies(&[]);

        // INIT
        let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
        let evm_merkle_roots = vec![ "evm_merkle_roots".to_string() ];
        let init_timestamp = 1_000_000_000;

        let msg = InstantiateMsg {
            mars_token_address: "mars_token_contract".to_string(),
            owner: "owner_address".to_string(),
            till_timestamp: init_timestamp + 1000,
            terra_merkle_roots: terra_merkle_roots.clone(),
            evm_merkle_roots: evm_merkle_roots.clone() 
        };

        let info = mock_info("owner_address", &[]);
        let env = mock_env_custom(MockEnvParams {
            block_time: Timestamp::from_seconds(init_timestamp),
            ..Default::default()
        });

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());        

        // WORKS (SENDER == owner)
        let msg = ExecuteMsg::Updateowner {
            new_owner: "new_owner_address".to_string(),
        };
        
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let query_res = query(deps.as_ref(), QueryMsg::Config {}).unwrap();
        let config_res: ConfigResponse = from_binary(&query_res).unwrap();
        assert_eq!("new_owner_address".to_string() , config_res.owner);

        // DOESN'T WORK (SENDER != owner)
        // let info = mock_info("owner_address", &[]);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone());

        assert_generic_error_message(res,"Sender not owner!" );
    }


    #[test]
    fn test_update_terra_merkle_roots() {

        let mut deps = mock_dependencies(&[]);

        // INIT
        let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
        let evm_merkle_roots = vec![ "evm_merkle_roots".to_string() ];
        let init_timestamp = 1_000_000_000;

        let msg = InstantiateMsg {
            mars_token_address: "mars_token_contract".to_string(),
            owner: "owner_address".to_string(),
            till_timestamp: init_timestamp + 1000,
            terra_merkle_roots: terra_merkle_roots.clone(),
            evm_merkle_roots: evm_merkle_roots.clone() 
        };

        let info = mock_info("owner_address", &[]);
        let env = mock_env_custom(MockEnvParams {
            block_time: Timestamp::from_seconds(init_timestamp),
            ..Default::default()
        });

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());        

        let new_terra_merkle_roots = vec!["new_terra_merkle_roots".to_string()]; 

        // WORKS (SENDER == owner)
        let msg = ExecuteMsg::UpdateTerraMerkleRoots {
            merkle_roots: new_terra_merkle_roots.clone(),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone(), ).unwrap();

        let query_res = query(deps.as_ref(), QueryMsg::Config {}).unwrap();
        let config_res: ConfigResponse = from_binary(&query_res).unwrap();
        assert_eq!(new_terra_merkle_roots.clone(), config_res.terra_merkle_roots);

        // DOESN'T WORK (SENDER != owner)
        let info = mock_info("wrong_owner_address", &[]);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone(), );
        assert_generic_error_message(res,"Sender not owner!" );
    }



    // #[test]
    fn test_update_evm_merkle_roots() {

        let mut deps = mock_dependencies(&[]);

        // INIT
        let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
        let evm_merkle_roots = vec![ "evm_merkle_roots".to_string() ];
        let init_timestamp = 1_000_000_000;

        let msg = InstantiateMsg {
            mars_token_address: "mars_token_contract".to_string(),
            owner: "owner_address".to_string(),
            till_timestamp: init_timestamp + 1000,
            terra_merkle_roots: terra_merkle_roots.clone(),
            evm_merkle_roots: evm_merkle_roots.clone() 
        };

        let info = mock_info("owner_address", &[]);
        let env = mock_env_custom(MockEnvParams {
            block_time: Timestamp::from_seconds(init_timestamp),
            ..Default::default()
        });

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();
        assert_eq!(0, res.messages.len());        

        let new_evm_merkle_roots = vec!["new_evm_merkle_roots".to_string()]; 

        // WORKS (SENDER == owner)
        let msg = ExecuteMsg::UpdateEvmMerkleRoots {
            merkle_roots: new_evm_merkle_roots.clone(),
        };
        execute(deps.as_mut(), env.clone(), info.clone(), msg.clone(), ).unwrap();

        let query_res = query(deps.as_ref(), QueryMsg::Config {}).unwrap();
        let config_res: ConfigResponse = from_binary(&query_res).unwrap();
        assert_eq!(new_evm_merkle_roots.clone(), config_res.evm_merkle_roots);

        // DOESN'T WORK (SENDER != owner)
        let info = mock_info("wrong_owner_address", &[]);
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone(), );
        assert_generic_error_message(res,"Sender not owner!" );
    }


    // #[test]
    // fn test_update_terra_user_claim() {

    //     let mut deps = mock_dependencies(&[]);

    //     // INIT
    //     let terra_merkle_roots = vec!["815cc797fb6186940e0f85a83da235e9b6342c9cc2830a5bd3ca10fd2947ed9c".to_string()];
    //     let evm_merkle_roots = vec![ "evm_merkle_roots".to_string() ];

    //     let msg = InitMsg {
    //         token: HumanAddr::from("mars_token_contract"),
    //         owner: HumanAddr::from("owner"),
    //         terra_merkle_roots: terra_merkle_roots.clone(),
    //         evm_merkle_roots: evm_merkle_roots.clone() 
    //     };

    //     let env = mock_env();

    //     // we can just call .unwrap() to assert this was a success
    //     let res = init(&mut deps, env, msg).unwrap();
    //     assert_eq!(0, res.messages.len());        
        

    //     // DOES NOT WORK (INCORRECT MERKLE PROOF)
    //     let account = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    //     let claim_amount = Uint128::from(100000u128);
    //     let merkle_proof = vec!["df2a7ea1c1acdc80a731e836d654ddbf2f24d636715be4188919c8680e041318".to_string(),
    //                             "9e0c8ccb935470d698b00917420343fd75a85373f4055995ceba08fc87450ae1".to_string()];
    //     let root_index = 0;

    //     let env = mock_env(account.clone(), &[]);
    //     let msg = ExecuteMsg::TerraClaim {
    //         amount: claim_amount,
    //         merkle_proof: merkle_proof.clone(),
    //         root_index: root_index
    //     };
    //     let res = handle(&mut deps, env, msg.clone());
    //     assert_generic_error_message(res,"Incorrect Merkle Proof" );

    //     let query_res = query(&deps, QueryMsg::IsClaimed { 
    //         address: account.to_string(),
    //     }).unwrap();
    //     let claim_res: ClaimResponse = from_binary(&query_res).unwrap();
    //     assert_eq!(false, claim_res.is_claimed);

    //     // DOES NOT WORK (INCORRECT MERKLE PROOF : SENT BY DIFFERENT USER)
    //     let account = "terra1x45rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    //     let claim_amount = Uint128::from(100000u128);
    //     let merkle_proof = vec!["df1a7ea1c1acdc80a731e836d654ddbf2f24d636715be4188919c8680e041318".to_string(),
    //                             "9e0c8ccb935470d698b00917420343fd75a85373f4055995ceba08fc87450ae1".to_string()];
    //     let root_index = 0;

    //     let env = mock_env(account.clone(), &[]);
    //     let msg = ExecuteMsg::TerraClaim {
    //         amount: claim_amount,
    //         merkle_proof: merkle_proof.clone(),
    //         root_index: root_index
    //     };
    //     let res = handle(&mut deps, env, msg.clone());
    //     assert_generic_error_message(res,"Incorrect Merkle Proof" );

    //     let query_res = query(&deps, QueryMsg::IsClaimed { 
    //         address: account.to_string(),
    //     }).unwrap();
    //     let claim_res: ClaimResponse = from_binary(&query_res).unwrap();
    //     assert_eq!(false, claim_res.is_claimed);

    //     // WORKS 
    //     let account = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    //     let claim_amount = Uint128::from(100000u128);
    //     let merkle_proof = vec!["df1a7ea1c1acdc80a731e836d654ddbf2f24d636715be4188919c8680e041318".to_string(),
    //                             "9e0c8ccb935470d698b00917420343fd75a85373f4055995ceba08fc87450ae1".to_string()];
    //     let root_index = 0;

    //     let env = mock_env(account.clone(), &[]);
    //     let msg = ExecuteMsg::TerraClaim {
    //         amount: claim_amount,
    //         merkle_proof: merkle_proof.clone(),
    //         root_index: root_index
    //     };
    //     let res = handle(&mut deps, env, msg.clone()).unwrap();
    //     assert_eq!(
    //         res.attributes,
    //         vec![
    //             attr("action", "claim_for_terra"),
    //             attr("claimed", "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v"),
    //             attr("amount", "100000"),
    //         ]
    //     );
    //     assert_eq!(
    //         res.messages,
    //         vec![ SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
    //                 contract_addr: HumanAddr::from("mars_token_contract"),
    //                 send: vec![],
    //                 msg: to_binary(&CW20ExecuteMsg::Transfer {
    //                     recipient: HumanAddr::from(account),
    //                     amount: claim_amount,
    //                 }).unwrap(),
    //         }))]
    //     );

    //     let query_res = query(&deps, QueryMsg::IsClaimed {  address: account.to_string() }).unwrap();
    //     let claim_res: ClaimResponse = from_binary(&query_res).unwrap();
    //     assert_eq!(true, claim_res.is_claimed);

    //     let env = mock_env(account.clone(), &[]);
    //     let res = handle(&mut deps, env, msg.clone());
    //     assert_generic_error_message(res,"Account has already claimed the Airdrop");
    // }



    // #[test]
    // fn test_update_evm_user_claim() {

    //     let mut deps = mock_dependencies(&[]);

    //     // INIT
    //     let terra_merkle_roots = vec!["terra_merkle_roots".to_string()];
    //     let evm_merkle_roots = vec![ "4af2617cdbb74cf2ad0c141cc3be98b1aafcbee7dcc709048a54ebb2cbf10c84".to_string() ];

    //     let msg = InitMsg {
    //         token: HumanAddr::from("mars_token_contract"),
    //         owner: HumanAddr::from("owner"),
    //         terra_merkle_roots: terra_merkle_roots.clone(),
    //         evm_merkle_roots: evm_merkle_roots.clone() 
    //     };

    //     let env = mock_env();

    //     // we can just call .unwrap() to assert this was a success
    //     let res = init(&mut deps, env, msg).unwrap();
    //     assert_eq!(0, res.messages.len());        
        

    //     // DOES NOT WORK (INCORRECT MERKLE PROOF)
    //     let recepient_account =  HumanAddr::from("recepient");
    //     let eth_account = "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d"; // Lower-case, without '0x' prefix
    //     let claim_amount = Uint128::from(240705475u128);
    //     let merkle_proof = vec!["a95c1b64e958b37a4c1321365b6ee909564727ea813824eef6479399b624c4e2".to_string(),
    //                             "e9e75b78466f5a65225a0b125c8455c7884352a6e1d05d3aa40ea0d969676363".to_string()];
    //     let signature = "3e265fbd1c16400fcbbf176f131c2cd32125d527fe5c4065fbcc77a48daea7111e9f09366154f915477b8a0f8748879b28f94c75d21a8c78cf0df202265b071b1c".to_string();
    //     let root_index = 0;

    //     let env = mock_env(recepient_account.clone(), &[]);
    //     let msg = ExecuteMsg::EvmClaim {
    //         eth_address: eth_account.to_string().clone(),
    //         claim_amount: claim_amount,
    //         signature: signature.clone(),
    //         merkle_proof: merkle_proof.clone(),
    //         root_index: root_index
    //     };
    //     let res = handle(&mut deps, env, msg.clone());
    //     assert_generic_error_message(res,"Incorrect Merkle Proof" );

    //     let query_res = query(&deps, QueryMsg::IsClaimed { 
    //         address: eth_account.to_string(),
    //     }).unwrap();
    //     let claim_res: ClaimResponse = from_binary(&query_res).unwrap();
    //     assert_eq!(false, claim_res.is_claimed);

    //     // DOES NOT WORK (INCORRECT SIGNATURE)
    //     let recepient_account =  HumanAddr::from("recepient");
    //     let eth_account =  "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d"; // Lower-case, without '0x' prefix
    //     let claim_amount = Uint128::from(240705475u128);
    //     let merkle_proof = vec!["a94c1b64e958b37a4c1321365b6ee909564727ea813824eef6479399b624c4e2".to_string(),
    //                             "e9e75b78466f5a65225a0b125c8455c7884352a6e1d05d3aa40ea0d969676363".to_string()];
    //     let signature = "3e265fbd1c16400fcbbf176f131c2cd32125d527fe5c4065fbcc77a48daea7111e9f09366154f915477b8a0f8748879b28f94c75d21a8c78cf0df202265b071b1c".to_string();
    //     let root_index = 0;

    //     let env = mock_env(recepient_account.clone(), &[]);
    //     let msg = ExecuteMsg::EvmClaim {
    //         eth_address: eth_account.to_string().clone(),
    //         claim_amount: claim_amount,
    //         signature: signature.clone(),
    //         merkle_proof: merkle_proof.clone(),
    //         root_index: root_index
    //     };
    //     let res = handle(&mut deps, env, msg.clone());
    //     assert_generic_error_message(res,"Invalid Signature" );

    //     let query_res = query(&deps, QueryMsg::IsClaimed { 
    //         address: eth_account.to_string(),
    //     }).unwrap();
    //     let claim_res: ClaimResponse = from_binary(&query_res).unwrap();
    //     assert_eq!(false, claim_res.is_claimed);

    //     // WORKS 
    //     let recepient_account =  HumanAddr::from("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v");
    //     let eth_account =  "2c21b6fa9f82892d9853d8ee2351dc3c3e8e176d"; // Lower-case, without '0x' prefix
    //     let claim_amount = Uint128::from(240705475u128);
    //     let merkle_proof = vec!["a94c1b64e958b37a4c1321365b6ee909564727ea813824eef6479399b624c4e2".to_string(),
    //                             "e9e75b78466f5a65225a0b125c8455c7884352a6e1d05d3aa40ea0d969676363".to_string()];
    //     let signature = "3e265fbd1c16400fcbbf176f131c2cd32125d527fe5c4065fbcc77a48daea7111e9f09366154f915477b8a0f8748879b28f94c75d21a8c78cf0df202265b071b1c".to_string();
    //     let root_index = 0;

    //     let env = mock_env(recepient_account.clone(), &[]);
    //     let msg = ExecuteMsg::EvmClaim {
    //         eth_address: eth_account.to_string().clone(),
    //         claim_amount: claim_amount,
    //         signature: signature.clone(),
    //         merkle_proof: merkle_proof.clone(),
    //         root_index: root_index
    //     };
    //     let res = handle(&mut deps, env, msg.clone()).unwrap();
    //     assert_eq!(
    //         res.attributes,
    //         vec![
    //             attr("action", "claim_for_evm"),
    //             attr("claimed", eth_account.to_string() ),
    //             attr("recepient", recepient_account.to_string() ),
    //             attr("amount", "240705475"),
    //         ]
    //     );
    //     assert_eq!(
    //         res.messages,
    //         vec![ SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
    //                 contract_addr: HumanAddr::from("mars_token_contract"),
    //                 send: vec![],
    //                 msg: to_binary(&CW20ExecuteMsg::Transfer {
    //                     recipient: recepient_account.clone(),
    //                     amount: claim_amount,
    //                 }).unwrap(),
    //         }))]
    //     );

    //     let query_res = query(&deps, QueryMsg::IsClaimed {  address: eth_account.to_string() }).unwrap();
    //     let claim_res: ClaimResponse = from_binary(&query_res).unwrap();
    //     assert_eq!(true, claim_res.is_claimed);

    //     let env = mock_env(recepient_account.clone(), &[]);
    //     let res = handle(&mut deps, env, msg.clone());
    //     assert_generic_error_message(res,"Account has already claimed the Airdrop");
    // }


    /// Assert StdError::GenericErr message with expected_msg
    pub fn assert_generic_error_message<T>(response: StdResult<T>, expected_msg: &str) {
        match response {
            Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, expected_msg),
            Err(other_err) => panic!("Unexpected error: {:?}", other_err),
            Ok(_) => panic!("SHOULD NOT ENTER HERE!"),
        }
    }

}