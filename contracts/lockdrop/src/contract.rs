#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, Uint128, QuerierWrapper,CosmosMsg, BankMsg, QueryRequest,WasmQuery, Addr, Coin, DepsMut, Env, MessageInfo, WasmMsg, Response, StdResult, StdError};
use cosmwasm_bignumber::{Decimal256, Uint256};

use crate::msg::{ExecuteMsg, InstantiateMsg, CallbackMsg, QueryMsg};
use crate::state::{Config, CONFIG, State, STATE, UserInfo, USER_INFO, LockupInfo, LOCKUP_INFO};

// use mars::{address_provider};

use mars::address_provider::helpers::{query_address};
use mars::address_provider::msg::MarsContract;

use mars::helpers::{cw20_get_balance, option_string_to_addr, zero_address};

const SECONDS_PER_YEAR: u64 = 31536000u64;

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate( deps: DepsMut,_env: Env,info: MessageInfo,msg: InstantiateMsg ) -> StdResult<Response> {

    let config = Config {
        owner: option_string_to_addr(deps.api, msg.owner, zero_address())?,
        address_provider: option_string_to_addr(deps.api, msg.address_provider, zero_address())?, 
        maUST_token: option_string_to_addr(deps.api, msg.maUST_token, zero_address())?, 
        init_timestamp: msg.init_timestamp.unwrap_or(0 as u64) ,
        min_lock_duration: msg.min_duration.unwrap_or(0 as u64) ,
        max_lock_duration: msg.max_duration.unwrap_or(0 as u64) ,
        weekly_multiplier: msg.multiplier.unwrap_or(Decimal256::zero()) ,
        denom: msg.denom.unwrap_or("uusd".to_string()) ,
        lockdrop_incentives: msg.lockdrop_incentives.unwrap_or(Uint256::zero()) 
    };

    let state = State {
        total_UST_locked: Uint256::zero(),
        total_maUST_locked: Uint256::zero(),
        global_reward_index: Decimal256::zero(),
    };

    CONFIG.save( deps.storage, &config)?;
    STATE.save( deps.storage, &state)?;
    Ok(Response::default())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        // ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::LockUST { duration } => try_lock_UST(deps, _env, info,  duration),
        ExecuteMsg::UnlockUST { duration } => try_unlock_UST(deps, _env, info, duration),
        ExecuteMsg::ClaimRewards { } => try_claim(deps, _env, info),
        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, _env, info, new_config),
        ExecuteMsg::Callback(msg) => _handle_callback(deps, _env, info, msg),
    }
}



fn _handle_callback(deps: DepsMut, env: Env, info: MessageInfo, msg: CallbackMsg) -> StdResult<Response> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }
    match msg {
        CallbackMsg::UpdateStateOnDeposit {
            user, 
            duration, 
            ust_deposited, 
            mUST_prev_balance
        } => update_state_on_deposit(deps, env, info, user, duration, ust_deposited, mUST_prev_balance),
        CallbackMsg::UpdateStateOnWithdraw {
            user,
            duration, 
            mUST_withdrawn, 
            prev_ust_balance
        } => update_state_on_withdraw(deps, env, info, user , duration, mUST_withdrawn, prev_ust_balance),
    }
}

// #[cfg_attr(not(feature = "library"), entry_point)]
// pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
//     match msg {
//         QueryMsg::GetConfig {} => to_binary(&query_config(deps)?),
//         QueryMsg::GetLockups { user } => to_binary(&query_lockups(deps, user)?),
//         QueryMsg::GetLockupInfo { user, duration } => to_binary(&query_lockup_info(deps, user, duration)?),
//     }
// }

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------


pub fn update_config( deps: DepsMut, env: Env, info: MessageInfo, new_config: InstantiateMsg ) -> StdResult<Response> { 

    let mut config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.address_provider = option_string_to_addr(deps.api, new_config.address_provider, config.address_provider)?;
    config.maUST_token = option_string_to_addr(deps.api, new_config.maUST_token, config.maUST_token)?;
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;

    // UPDATE :: VALUES IF PROVIDED
    config.init_timestamp = new_config.init_timestamp.unwrap_or(config.init_timestamp);
    config.min_lock_duration = new_config.min_duration.unwrap_or(config.min_lock_duration);
    config.max_lock_duration = new_config.max_duration.unwrap_or(config.max_lock_duration);
    config.denom = new_config.denom.unwrap_or(config.denom );
    config.weekly_multiplier = new_config.multiplier.unwrap_or(config.weekly_multiplier);
    config.lockdrop_incentives = new_config.lockdrop_incentives.unwrap_or(config.lockdrop_incentives);


    Ok(Response::new().add_attribute("action", "lockdrop::ExecuteMsg::UpdateConfig"))
}


// USER SENDS UST --> CONTRACT DEPOSITS IT INTO RED BANK --> USER'S LOCKUP POSITION IS UPDATED
pub fn try_lock_UST( deps: DepsMut, env: Env, info: MessageInfo, duration: u64 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // Get UST deposit amount
    let deposit_amount = get_denom_amount_from_coins(&info.funds, &config.denom);
    let depositor_address = info.sender.clone();

    // CHECK :: Lockdrop deposit window open
    if config.init_timestamp < env.block.time.seconds() {
        return Err(StdError::generic_err("Lockdrop window is closed"));
    }

    // CHECK :: Valid Deposit Amount 
    if deposit_amount == Uint256::zero() {
        return Err(StdError::generic_err("Amount cannot be zero"));
    }

    // CHECK :: Valid Lockup Duration
    if duration > config.max_lock_duration || duration < config.min_lock_duration {
        return Err(StdError::generic_err(format!("Lockup duration needs to be between {} and {}",config.min_lock_duration,config.max_lock_duration)));
    }

    // QUERY :: CURRENT maUST BALANCE
    let mUST_balance = Uint256::from(cw20_get_balance(&deps.querier, config.maUST_token.clone(), env.contract.address.clone() )?);
    let red_bank = query_address( &deps.querier,config.address_provider, MarsContract::RedBank )?;

    // let mut messages: Vec<CosmosMsg> = vec![];
    // messages.extend(iter)

    // COSMOS_MSG :: DEPOSIT UST IN RED BANK
    let redbank_deposit_msg = build_deposit_into_redbank_msg(red_bank, config.denom.clone(), deposit_amount)?;

    // COSMOS_MSG :: UPDATE CONTRACT STATE
    let updateState_msg = CallbackMsg::UpdateStateOnDeposit {   
        user: depositor_address.clone(),
        duration: duration.clone(),
        ust_deposited: deposit_amount.clone(),
        mUST_prev_balance: mUST_balance
    }.to_cosmos_msg(&env.contract.address)?;


    Ok(Response::new()
    .add_messages(vec![redbank_deposit_msg, updateState_msg])
    .add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::LockUST"),
        ("user", &depositor_address.to_string()),
        ("duration", duration.to_string().as_str()),
        ("ust_amount", deposit_amount.to_string().as_str()),
    ]))
}


// USER UNLOCKS UST --> CONTRACT WITHDRAWS FROM RED BANK --> STATE UPDATED VIA EXTEND MSG
pub fn try_unlock_UST( deps: DepsMut, env: Env, info: MessageInfo, duration: u64 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let depositor_address = info.sender.clone();
    let current_timestamp = env.block.time.seconds();

    // LOCKUP INFO :: RETRIEVE
    let lockup_id = depositor_address.to_string() + &duration.to_string();
    let lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.as_bytes() )?.unwrap_or_default();

    // CHECK :: IS VALID LOCKUP
    if lockup_info.ma_UST_locked == Uint256::zero() {
        return Err(StdError::generic_err("No lockup to unlock"));
    }    

    // CHECK :: LOCKUP CAN BE UNLOCKED 
    if lockup_info.unlock_timestamp > current_timestamp.clone() {
        let time_remaining = lockup_info.unlock_timestamp - current_timestamp.clone();
        return Err(StdError::generic_err(format!("{} seconds to Unlock",time_remaining)));
    }

    // CONTRACT :: CURRENT UST BALANCE
    let ust_balance = Uint256::from( deps.querier.query_balance(env.contract.address.clone(), config.denom.as_str())?.amount );
    let red_bank = query_address( &deps.querier,config.address_provider, MarsContract::RedBank )?;

    // COSMOS_MSG :: WITHDRAW UST FROM RED BANK
    let redbank_withdraw_msg = build_withdraw_from_redbank_msg(red_bank, config.denom.clone(), lockup_info.ma_UST_locked.clone())?;

    // COSMOS_MSG :: UPDATE CONTRACT STATE
    let updateState_msg = CallbackMsg::UpdateStateOnWithdraw {   
        user: depositor_address.clone(),
        duration: duration.clone(),
        mUST_withdrawn: lockup_info.ma_UST_locked.clone(),
        prev_ust_balance: ust_balance
    }.to_cosmos_msg(&env.contract.address)?;

    Ok(Response::new()
        .add_messages(vec![redbank_withdraw_msg, updateState_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::UnlockUST"),
            ("owner", info.sender.as_str()),
            ("duration", duration.to_string().as_str()),
            ("amount", lockup_info.ma_UST_locked.to_string().as_str()),
        ]))
}



// USER CLAIMS REWARDS ACROSS ALL HIS LOCKUP POSITIONS
pub fn try_claim( deps: DepsMut, env: Env, info: MessageInfo ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let depositor_address = info.sender.clone();
    let current_timestamp = env.block.time.seconds();

    // CHECK :: REWARDS CAN BE CLAIMED
    if config.init_timestamp > current_timestamp.clone() {
        return Err(StdError::generic_err("Claim not allowed during deposit window"));
    }    

    // USER INFO :: RETRIEVE --> UPDATE --> SAVE
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address )?.unwrap_or_default();
    let mars_token = query_address( &deps.querier,config.address_provider.clone(), MarsContract::MarsToken )?;
    let incentives_contract = query_address( &deps.querier,config.address_provider, MarsContract::Incentives )?;

    // CHECK :: HAS VALID LOCKUP POSITIONS
    if user_info.total_ma_UST_locked == Uint256::zero() {
        return Err(StdError::generic_err("No valid lockup found"));
    }

    // COMPUTE :: GLOBALLY ACCRUED DEPOSIT INCENTIVES
    compute_accrued_reward(&deps.querier, env, incentives_contract, config.maUST_token, &mut state);           
    
    // LOCKDROP :: $MARS Rewards
    let mut total_rewards = Uint256::zero();
    let total_lockdrop_incentives = config.lockdrop_incentives;

    // LOOP OVER ALL LOCKUP POSITIONS :: UPDATE EACH POSITION
    for lockupId in &mut user_info.lockup_positions {

        let mut rewards = Uint256::zero();
        let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockupId.as_bytes())?.unwrap_or_default();

        // TO BE CLAIMED :::: CALCULATE LOCKDROP REWARD
        if !lockup_info.lockdrop_claimed {
            rewards = calculate_lockdrop_reward(lockup_info.ma_UST_locked , lockup_info.duration, total_lockdrop_incentives.clone(), config.weekly_multiplier);
            lockup_info.lockdrop_reward = rewards;
            lockup_info.lockdrop_claimed = true;
        }
        
        // TO BE CLAIMED :::: CALCULATE ACCRUED DEPOSIT INCENTIVES
        compute_staker_accrued_reward(state.clone(), &mut lockup_info);        
        rewards += lockup_info.pending_reward;  
        lockup_info.pending_reward = Uint256::zero();

        // TO BE CLAIMED :::: ADD TO TOTAL REWARDS ACCRUED
        total_rewards += rewards;

        // LOCKUP INFO :: SAVE UPDATED STATE
        LOCKUP_INFO.save(deps.storage, lockupId.as_bytes(), &lockup_info);
    }

    if total_rewards == Uint256::zero() {
        return Err(StdError::generic_err("No rewards to claim"));
    }

    // COSMOS_MSG :: CLAIM REWARDS
    let transfer_mars_msg = build_send_cw20_token_msg(depositor_address.clone(), mars_token, total_rewards)?;

    Ok(Response::new()
        .add_messages(vec![transfer_mars_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::ClaimRewards"),
            ("owner", depositor_address.clone().as_str()),
            ("rewards_claimed", total_rewards.to_string().as_str()),
        ]))    
}


//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------



// CALLBACK :: CALLED BY LOCK_UST FUNCTION --> UPDATES STATE :: STATE, USER'S LOCKUP POSITION, USER INFO
pub fn update_state_on_deposit( deps: DepsMut, env: Env, info: MessageInfo, user: Addr, duration: u64, deposit_amount:Uint256, mUST_prev_balance: Uint256 ) -> StdResult<Response> { 

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let mUST_balance = Uint256::from(cw20_get_balance(&deps.querier, config.maUST_token.clone(), env.contract.address.clone() )?);
    let mUST_minted = mUST_balance - mUST_prev_balance;
    
    // STATE :: UPDATE
    state.total_UST_locked += deposit_amount;
    state.total_maUST_locked =  mUST_balance;

    // LOCKUP INFO :: RETRIEVE --> UPDATE --> SAVE
    let lockup_id = user.clone().to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes() )?.unwrap_or_default();
    lockup_info.user = user.clone();
    lockup_info.duration = duration;
    lockup_info.ma_UST_locked += mUST_minted;
    lockup_info.unlock_timestamp = config.init_timestamp + (duration*(86400 as u64));

    // USER INFO :: RETRIEVE --> UPDATE --> SAVE
    let mut user_info = USER_INFO.may_load(deps.storage, &user.clone() )?.unwrap_or_default();
    user_info.total_ust_locked += deposit_amount;
    user_info.total_ma_UST_locked += mUST_minted;
    user_info.lockup_positions.push(lockup_id.clone() );

    STATE.save(deps.storage, &state);
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &user, &user_info)?;

    Ok(Response::new()
    .add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::UpdateState"),
        ("user", &user.to_string()),
        ("duration", duration.to_string().as_str()),
        ("ust_deposited", deposit_amount.to_string().as_str()),
        ("maUST_minted", mUST_minted.to_string().as_str()),
    ]))

}


// CALLBACK :: CALLED BY UNLOCK_UST FUNCTION --> UPDATES STATE :: STATE, DELETES LOCKUP POSITION, TRANSFERS UST, $MARS REWARDS
pub fn update_state_on_withdraw( deps: DepsMut, env: Env, info: MessageInfo, user: Addr, duration: u64, withdraw_amount_maUST:Uint256, prev_ust_balance: Uint256 ) -> StdResult<Response> { 

    // CHECK :: Only the contract itself can call this fn
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // USER INFO :: RETRIEVE 
    let mut user_info = USER_INFO.may_load(deps.storage, &user )?.unwrap_or_default();

    // LOCKUP INFO :: RETRIEVE
    let lockup_id = user.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.as_bytes() )?.unwrap_or_default();

    // CALCULATE :: UST UNLOCKED
    let cur_ust_balance = Uint256::from( deps.querier.query_balance(env.contract.address.clone(), config.denom.as_str())?.amount );
    let ust_withdrawn = cur_ust_balance - prev_ust_balance;
    let mars_token = query_address( &deps.querier,config.address_provider.clone(), MarsContract::MarsToken )?;
    let incentives_contract = query_address( &deps.querier,config.address_provider, MarsContract::Incentives )?;

    let mut pending_rewards = Uint256::zero();

    // TO BE CLAIMED ?? :::: CALCULATE LOCKDROP REWARD
    if !lockup_info.lockdrop_claimed {
        let rewards = calculate_lockdrop_reward(lockup_info.ma_UST_locked , lockup_info.duration, config.lockdrop_incentives, config.weekly_multiplier);
        lockup_info.lockdrop_reward = rewards;
        pending_rewards += rewards;
        lockup_info.lockdrop_claimed = true;
    }

 
    compute_accrued_reward(&deps.querier, env, incentives_contract, config.maUST_token , &mut state);            // Compute global reward 
    compute_staker_accrued_reward(state.clone(), &mut lockup_info);           // Compute depositor reward

    // UNCLAIMED REWARDS
    pending_rewards += lockup_info.pending_reward;

    // UPDATE STATE
    state.total_UST_locked = state.total_UST_locked - ust_withdrawn;
    // let total_maUST_locked = state.total_maUST_locked.clone();
    state.total_maUST_locked = state.total_maUST_locked - withdraw_amount_maUST;

    // UPDATE USER INFO
    user_info.total_ma_UST_locked = user_info.total_ma_UST_locked - withdraw_amount_maUST;
    user_info.total_ust_locked = user_info.total_ust_locked - ust_withdrawn;

    // REMOVE LOCKUP INFO FROM lockup_positions array IN USER INFO
    let index = user_info.lockup_positions.iter().position(|x| *x == lockup_id).unwrap();
    user_info.lockup_positions.remove(index);

    STATE.save(deps.storage, &state);

    if user_info.total_ma_UST_locked == Uint256::zero() {
        USER_INFO.remove(deps.storage, &user);
    } else {
        USER_INFO.save(deps.storage, &user, &user_info)?;
    }

    // REMOVE LOCKUP DETAILS
    LOCKUP_INFO.remove(deps.storage, lockup_id.as_bytes());

    let mut messages = vec![];

    // COSMOS_MSG ::TRANSFER WITHDRAWN UST
    messages.push( build_send_native_asset_msg(user.clone(), &config.denom.clone(), ust_withdrawn)?  );

    // COSMOS_MSG ::TRANSFER ACCUMULATED $MARS REWARDS
    if pending_rewards > Uint256::zero() {
        let transfer_mars_msg = build_send_cw20_token_msg(user.clone(), mars_token, pending_rewards)?;
        messages.push(transfer_mars_msg);
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::Callback_UpdateStateOnWithdraw"),
            ("user", user.clone().as_str()),
            ("duration", duration.to_string().as_str()),
            ("ust_withdrawn", ust_withdrawn.to_string().as_str()),
            ("rewards_claimed", pending_rewards.to_string().as_str())
        ]))
}


//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

// Calculate Lockdrop Reward
fn calculate_lockdrop_reward(deposit_amount:Uint256, duration: u64, total_rewards: Uint256, weekly_multiplier:Decimal256) -> Uint256 {
    let _multiplier = Decimal256::from_ratio(duration, 7 as u64) * weekly_multiplier;
    Decimal256::from_uint256(deposit_amount) * _multiplier * total_rewards
}

// native coins
fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

//-----------------------------
// MARS REWARDS COMPUTATION
//-----------------------------

// MARS REWARDS COMPUTATION

// Accrue MARS reward by updating the reward index
fn compute_accrued_reward(querier: &QuerierWrapper, env:Env, incentives_contract: Addr, maUST_token: Addr, state: &mut State) {

    // let incentives_contract = query_address( querier,config.address_provider, MarsContract::Incentives )?;

    // Get MARS reward accrued by the contract
    let accrued_reward: Uint128 = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: incentives_contract.to_string(),
        msg: to_binary(&mars::incentives::msg::QueryMsg::UserUnclaimedRewards {
            user_address: env.contract.address.to_string(),
        }).unwrap(),
    })).unwrap();   
    
    // Get maUST Balance
    let maUST_balance: cw20::BalanceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: maUST_token.to_string(),
        msg: to_binary(&mars::ma_token::msg::QueryMsg::Balance {
            address: env.contract.address.to_string(),
        }).unwrap(),
    })).unwrap();

    let accrued_index = Decimal256::from_ratio(Uint256::from(accrued_reward) , Uint256::from(maUST_balance.balance) );
    state.global_reward_index = state.global_reward_index + accrued_index;
} 

// Accrue MARS reward for the user by updating the user reward index and adding rewards to the pending rewards
fn compute_staker_accrued_reward(state: State, lockupInfo: &mut LockupInfo) { 
    let pending_reward = (lockupInfo.ma_UST_locked * state.global_reward_index) - (lockupInfo.ma_UST_locked * lockupInfo.reward_index);
    lockupInfo.reward_index = state.global_reward_index;
    lockupInfo.pending_reward += pending_reward;
}

//-----------------------------
// COSMOS_MSGs     
//-----------------------------


fn build_send_native_asset_msg( recipient: Addr, denom: &str, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![ Coin {
                        denom: denom.to_string(),
                        amount: amount.into(),
                }],
        }))
    }
     
fn build_send_cw20_token_msg(recipient: Addr, token_contract_address: Addr, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address.into(),
        msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount: amount.into(),
        })?,
        funds: vec![],
    }))
}
     
fn build_deposit_into_redbank_msg(redbank_address: Addr, denom_stable: String, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: redbank_address.to_string(),
            funds: vec![ Coin { denom: denom_stable.clone(), amount: amount.into() } ],
            msg: to_binary(&mars::red_bank::msg::ExecuteMsg::DepositNative {
                denom: denom_stable,
            })?,
    }))
}

fn build_withdraw_from_redbank_msg(redbank_address: Addr, denom_stable: String, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: redbank_address.to_string(),
        funds: vec![],
        msg: to_binary(&mars::red_bank::msg::ExecuteMsg::Withdraw {
            asset: mars::asset::Asset::Native { denom: denom_stable },
            amount: Some(amount),
        })?,
    }))
}

























































