#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary,Deps, QuerierWrapper,CosmosMsg, BankMsg, QueryRequest,WasmQuery, Addr, Coin, DepsMut, Env, MessageInfo, WasmMsg, Response, StdResult, StdError};
use cosmwasm_bignumber::{Decimal256, Uint256};

use crate::msg::{ExecuteMsg, InstantiateMsg, UpdateConfigMsg, CallbackMsg, QueryMsg, ConfigResponse,GlobalStateResponse, UserInfoResponse, LockUpInfoResponse };
use crate::state::{Config, CONFIG, State, STATE, UserInfo, USER_INFO, LockupInfo, LOCKUP_INFO};

use mars::address_provider::helpers::{query_address, query_addresses};
use mars::address_provider::msg::MarsContract;
use mars::helpers::{cw20_get_balance, option_string_to_addr, zero_address};

const SECONDS_PER_WEEK: u64 = 864 as u64;       // 7*86400 as u64;

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate( deps: DepsMut,_env: Env,info: MessageInfo,msg: InstantiateMsg ) -> StdResult<Response> {

    // CHECK :: init_timestamp needs to be valid
    if msg.init_timestamp < _env.block.time.seconds() {
        return Err(StdError::generic_err("Invalid timestamp"));
    }
    
    // CHECK :: deposit_window,withdrawal_window need to be valid (withdrawal_window < deposit_window)
    if msg.deposit_window == 0u64 || msg.withdrawal_window == 0u64 || msg.deposit_window <= msg.withdrawal_window {
        return Err(StdError::generic_err("Invalid deposit / withdraw window"));
    }

    // CHECK :: min_lock_duration , max_lock_duration need to be valid (min_lock_duration < max_lock_duration)
    if msg.max_duration <= msg.min_duration {
        return Err(StdError::generic_err("Invalid Lockup durations"));
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        address_provider: option_string_to_addr(deps.api, msg.address_provider, zero_address())?, 
        ma_ust_token: option_string_to_addr(deps.api, msg.ma_ust_token, zero_address())?, 
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
        min_lock_duration: msg.min_duration,
        max_lock_duration: msg.max_duration,
        weekly_multiplier: msg.weekly_multiplier.unwrap_or(Decimal256::zero()) ,
        denom: msg.denom.unwrap_or("uusd".to_string()) ,
        lockdrop_incentives: msg.lockdrop_incentives.unwrap_or(Uint256::zero()) 
    };

    let state = State {
        final_ust_locked: Uint256::zero(),
        final_maust_locked: Uint256::zero(),
        total_ust_locked: Uint256::zero(),
        total_maust_locked: Uint256::zero(),
        total_deposits_weight: Uint256::zero(),
        global_reward_index: Decimal256::zero(),
    };

    CONFIG.save( deps.storage, &config)?;
    STATE.save( deps.storage, &state)?;
    Ok(Response::default())
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, _env, info, new_config),
        ExecuteMsg::DepositUst { duration } => try_deposit_ust(deps, _env, info,  duration),
        ExecuteMsg::WithdrawUst { duration, amount } => try_withdraw_ust(deps, _env, info,  duration, amount),
        ExecuteMsg::ClaimRewards { } => try_claim(deps, _env, info),
        ExecuteMsg::Unlock { duration } => try_unlock_position(deps, _env, info, duration),
        ExecuteMsg::Callback(msg) => _handle_callback(deps, _env, info, msg),
    }
}



fn _handle_callback(deps: DepsMut, env: Env, info: MessageInfo, msg: CallbackMsg) -> StdResult<Response> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }
    match msg {
        CallbackMsg::UpdateStateOnRedBankDeposit {
            prev_ma_ust_balance
        } => update_state_on_red_bank_deposit(deps, env,  prev_ma_ust_balance),
        CallbackMsg::UpdateStateOnClaim {
            user,
            prev_xmars_balance
        } => update_state_on_claim(deps, env,  user , prev_xmars_balance),        
        CallbackMsg::DissolvePosition {
            user,
            duration
        } => try_dissolve_position(deps, env,  user , duration),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::UserInfo {address} => to_binary(&query_user_info(deps, address)?),
        QueryMsg::LockUpInfo {address , duration } => to_binary(&query_lockup_info(deps, address, duration)?),
        QueryMsg::LockUpInfoWithId { lockup_id } => to_binary(&query_lockup_info_with_id(deps, lockup_id)?),
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------


pub fn update_config( deps: DepsMut, env: Env, info: MessageInfo, new_config: UpdateConfigMsg ) -> StdResult<Response> { 

    let mut config = CONFIG.load(deps.storage)?;
    
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.address_provider = option_string_to_addr(deps.api, new_config.address_provider, config.address_provider)?;
    config.ma_ust_token = option_string_to_addr(deps.api, new_config.ma_ust_token, config.ma_ust_token)?;
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;

    // UPDATE :: init_timestamp (if provided) :: ALLOWED BEFORE THE LOCKUP DEPOSIT WINDOW OPENS 
    if env.block.time.seconds() < config.init_timestamp {
        config.init_timestamp = new_config.init_timestamp.unwrap_or(config.init_timestamp);
        config.min_lock_duration = new_config.min_duration.unwrap_or(config.min_lock_duration);
        config.max_lock_duration = new_config.max_duration.unwrap_or(config.max_lock_duration);
        config.weekly_multiplier = new_config.weekly_multiplier.unwrap_or(config.weekly_multiplier);
    }

    // LOCKDROP INCENTIVES :: CAN ONLY BE INCREASED
    if config.lockdrop_incentives < new_config.lockdrop_incentives.unwrap_or(Uint256::zero() ) {
        config.lockdrop_incentives = new_config.lockdrop_incentives.unwrap_or(config.lockdrop_incentives );
    }

    Ok(Response::new().add_attribute("action", "lockdrop::ExecuteMsg::UpdateConfig"))
}



// USER SENDS UST --> USER'S LOCKUP POSITION IS UPDATED
pub fn try_deposit_ust( deps: DepsMut, env: Env, info: MessageInfo, duration: u64 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // UST DEPOSITED & USER ADDRESS
    let deposit_amount = get_denom_amount_from_coins(&info.funds, &config.denom);
    let depositor_address = info.sender.clone();

    // CHECK :: Lockdrop deposit window open
    if !is_deposit_open(env.block.time.seconds(), &config ) {
        return Err(StdError::generic_err("Deposit window closed"));
    }

    // CHECK :: Valid Deposit Amount 
    if deposit_amount == Uint256::zero() {
        return Err(StdError::generic_err("Amount cannot be zero"));
    }

    // CHECK :: Valid Lockup Duration
    if duration > config.max_lock_duration || duration < config.min_lock_duration {
        return Err(StdError::generic_err(format!("Lockup duration needs to be between {} and {}",config.min_lock_duration,config.max_lock_duration)));
    }
    
    // LOCKUP INFO :: RETRIEVE --> UPDATE 
    let lockup_id = depositor_address.clone().to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes() )?.unwrap_or_default();
    lockup_info.ust_locked += deposit_amount;
    lockup_info.duration = duration;
    lockup_info.unlock_timestamp = calculate_unlock_timestamp(&config, duration);

    // USER INFO :: RETRIEVE --> UPDATE 
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address.clone() )?.unwrap_or_default();
    user_info.total_ust_locked += deposit_amount;
    if !is_lockup_present_in_user_info(&user_info, lockup_id.clone()) {
        user_info.lockup_positions.push(lockup_id.clone() );
    }

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked += deposit_amount;
    state.total_deposits_weight += calculate_weight(deposit_amount, duration, config.weekly_multiplier);


    STATE.save(deps.storage, &state)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    Ok(Response::new()
    .add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::LockUST"),
        ("user", &depositor_address.to_string()),
        ("duration", duration.to_string().as_str()),
        ("ust_deposited", deposit_amount.to_string().as_str()),
        ("total_ust_in_lockup", lockup_info.ust_locked.to_string().as_str()),
        ("total_ust_deposited_by_user", user_info.total_ust_locked.to_string().as_str())
    ]))
}


// USER WITHDRAWS UST --> USER'S LOCKUP POSITION IS UPDATED
pub fn try_withdraw_ust( deps: DepsMut, env: Env, info: MessageInfo, duration:u64, withdraw_amount: Uint256 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // USER ADDRESS AND LOCKUP DETAILS
    let withdrawer_address = info.sender.clone();
    let lockup_id = withdrawer_address.clone().to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes() )?.unwrap_or_default();

    // CHECK :: Lockdrop withdrawal window open
    if !is_withdraw_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Withdrawals not allowed"));
    }

    // CHECK :: Valid Lockup 
    if lockup_info.ust_locked == Uint256::zero()  {
        return Err(StdError::generic_err("Lockup doesn't exist"));
    }

    // CHECK :: Valid Withdraw Amount 
    if withdraw_amount == Uint256::zero() || withdraw_amount > lockup_info.ust_locked {
        return Err(StdError::generic_err("Invalid withdrawal request"));
    }
    
    // LOCKUP INFO :: RETRIEVE --> UPDATE 
    lockup_info.ust_locked = lockup_info.ust_locked - withdraw_amount;

    // USER INFO :: RETRIEVE --> UPDATE 
    let mut user_info = USER_INFO.may_load(deps.storage, &withdrawer_address.clone() )?.unwrap_or_default();
    user_info.total_ust_locked = user_info.total_ust_locked - withdraw_amount;
    if lockup_info.ust_locked == Uint256::zero() {
    remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());
    }

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked = state.total_ust_locked - withdraw_amount;
    state.total_deposits_weight = state.total_deposits_weight - calculate_weight(withdraw_amount, duration, config.weekly_multiplier);

    STATE.save(deps.storage, &state)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?; 
    USER_INFO.save(deps.storage, &withdrawer_address, &user_info)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN UST
    let withdraw_msg =  build_send_native_asset_msg(withdrawer_address.clone(), &config.denom.clone(), withdraw_amount)? ;

    Ok(Response::new()
    .add_messages(vec![withdraw_msg])
    .add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::WithdrawUST"),
        ("user", &withdrawer_address.to_string()),
        ("duration", duration.to_string().as_str()),
        ("ust_withdrawn", withdraw_amount.to_string().as_str()),
        ("total_ust_in_lockup", lockup_info.ust_locked.to_string().as_str()),
        ("total_ust_deposited_by_user", user_info.total_ust_locked.to_string().as_str())
    ]))
}


// ADMIN FUNCTION :: DEPOSITS UST INTO THE RED BANK AND UPDATES STATE VIA THE CALLBANK FUNCTION
pub fn try_deposit_in_red_bank( deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only Owner can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Lockdrop deposit window should be closed
    if env.block.time.seconds() < config.init_timestamp || is_deposit_open(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Lockdrop deposit window open"));
    }    

    // CHECK :: Revert in-case funds have already been deposited in red-bank
    if state.final_maust_locked > Uint256::zero() {
        return Err(StdError::generic_err("Already deposited"));
    }

    // FETCH CURRENT BALANCE, PREPARE DEPOSIT MSG 
    let red_bank = query_address( &deps.querier,config.address_provider, MarsContract::RedBank )?;
    let ma_ust_balance = Uint256::from(cw20_get_balance(&deps.querier, config.ma_ust_token.clone(), env.contract.address.clone() )?);
    let deposit_msg = build_deposit_into_redbank_msg( red_bank, config.denom.clone(), state.total_ust_locked )?;

    // COSMOS_MSG :: UPDATE CONTRACT STATE
    let update_state_msg = CallbackMsg::UpdateStateOnRedBankDeposit {   
        prev_ma_ust_balance: ma_ust_balance
    }.to_cosmos_msg(&env.contract.address)?;


    Ok(Response::new()
    .add_messages(vec![deposit_msg, update_state_msg])
    .add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
        ("ust_deposited_in_red_bank", state.total_ust_locked.to_string().as_str()),
        ("timestamp", env.block.time.seconds().to_string().as_str()),
    ]))
}


// USER CLAIMS REWARDS ::: claim xMARS --> UpdateStateOnClaim(callback)
pub fn try_claim( deps: DepsMut, env: Env, info: MessageInfo ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let user_address = info.sender.clone();
    let user_info = USER_INFO.may_load(deps.storage, &user_address )?.unwrap_or_default();

    // CHECK :: REWARDS CAN BE CLAIMED 
    if is_deposit_open(env.block.time.seconds(), &config)  {
        return Err(StdError::generic_err("Claim not allowed"));
    }    
     
    // CHECK :: HAS VALID LOCKUP POSITIONS
    if user_info.total_ust_locked == Uint256::zero() {
        return Err(StdError::generic_err("No lockup to claim rewards for"));
    }

    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::Incentives, MarsContract::XMarsToken];
    let mut addresses_query = query_addresses(&deps.querier,config.address_provider, mars_contracts)?;
    let xmars_address = addresses_query.pop().unwrap();
    let incentives_address = addresses_query.pop().unwrap();

    // Get XMARS Balance
    let xmars_balance: cw20::BalanceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                                                                contract_addr: xmars_address.to_string(),
                                                                msg: to_binary(&mars::xmars_token::msg::QueryMsg::Balance {
                                                                    address: env.contract.address.to_string(),
                                                                }).unwrap(),
                                                            })).unwrap();

    // COSMOS MSG's :: CLAIM XMARS REWARDS, UPDATE STATE VIA CALLBACK
    let claim_xmars_msg = build_claim_xmars_rewards(incentives_address.clone())?;
    let callback_msg = CallbackMsg::UpdateStateOnClaim {   
                            user: user_address,
                            prev_xmars_balance: xmars_balance.balance.into()
                        }.to_cosmos_msg(&env.contract.address)?;



    Ok(Response::new()
        .add_messages(vec![claim_xmars_msg, callback_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::ClaimRewards")
        ]))    
}



// USER UNLOCKS UST --> CONTRACT WITHDRAWS FROM RED BANK --> STATE UPDATED VIA EXTEND MSG
pub fn try_unlock_position( deps: DepsMut, env: Env, info: MessageInfo, duration: u64 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();

    // LOCKUP INFO :: RETRIEVE
    let lockup_id = depositor_address.clone().to_string() + &duration.to_string();
    let lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.as_bytes() )?.unwrap_or_default();

    // CHECK :: IS VALID LOCKUP
    if lockup_info.ust_locked == Uint256::zero() {
        return Err(StdError::generic_err("Invalid lockup"));
    }    

    // CHECK :: LOCKUP CAN BE UNLOCKED 
    if lockup_info.unlock_timestamp > env.block.time.seconds() {
        let time_remaining = lockup_info.unlock_timestamp - env.block.time.seconds();
        return Err(StdError::generic_err(format!("{} seconds to Unlock",time_remaining)));
    }

    // MaUST :: AMOUNT TO BE SENT TO THE USER
    let maust_unlocked = calculate_user_ma_ust_share(lockup_info.ust_locked, state.final_ust_locked, state.final_maust_locked );

    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::RedBank, MarsContract::Incentives, MarsContract::XMarsToken];
    let mut addresses_query = query_addresses(&deps.querier,config.address_provider, mars_contracts)?;
    let xmars_address = addresses_query.pop().unwrap();
    let incentives_address = addresses_query.pop().unwrap();

    // QUERY :: XMARS Balance
    let xmars_balance: cw20::BalanceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: xmars_address.to_string(),
        msg: to_binary(&mars::xmars_token::msg::QueryMsg::Balance {
            address: env.contract.address.to_string(),
        }).unwrap(),
    })).unwrap();

    // COSMOS MSG :: CLAIM XMARS REWARDS
    let claim_xmars_msg = build_claim_xmars_rewards(incentives_address.clone())?;

    // CALLBACK MSG :: UPDATE STATE ON CLAIM (before dissolving lockup position)
    let callback_claim_xmars_msg = CallbackMsg::UpdateStateOnClaim {   
                                                            user: depositor_address.clone(),
                                                            prev_xmars_balance: xmars_balance.balance.into()
                                                        }.to_cosmos_msg(&env.contract.address)?;


    let callback_dissolve_position_msg = CallbackMsg::DissolvePosition {   
                                            user: depositor_address.clone(),
                                            duration: duration
                                        }.to_cosmos_msg(&env.contract.address)?;

    // COSMOS MSG :: TRANSFER USER POSITION's MA-UST SHARE
    let maust_transfer_msg = build_send_cw20_token_msg(depositor_address.clone(), config.ma_ust_token, maust_unlocked )?;

    Ok(Response::new()
        .add_messages(vec![claim_xmars_msg, callback_claim_xmars_msg, callback_dissolve_position_msg, maust_transfer_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::UnlockPosition"),
            ("owner", info.sender.as_str()),
            ("duration", duration.to_string().as_str()),
            ("maUST_unlocked", maust_unlocked.to_string().as_str()),
        ]))
}


//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------


// CALLBACK :: CALLED AFTER UST DEPOSITED INTO RED BANK --> UPDATES CONTRACT STATE 
pub fn update_state_on_red_bank_deposit( deps: DepsMut, env: Env, prev_ma_ust_balance: Uint256 ) -> StdResult<Response> { 

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let cur_ma_ust_balance = Uint256::from(cw20_get_balance(&deps.querier, config.ma_ust_token.clone(), env.contract.address.clone() )?);
    let m_ust_minted = cur_ma_ust_balance - prev_ma_ust_balance;
    
    // STATE :: UPDATE --> SAVE
    state.final_ust_locked =  state.total_ust_locked;
    state.final_maust_locked =  m_ust_minted;
    state.total_ust_locked = Uint256::zero();
    state.total_maust_locked =  m_ust_minted;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
    .add_attributes(vec![
        ("action", "lockdrop::CallbackMsg::RedBankDeposit"),
        ("maUST_minted", m_ust_minted.to_string().as_str()),
    ]))
}


// CALLBACK :: CALLED AFTER XMARS CLAIMED BY CONTRACT --> TRANSFER REWARDS (MARS, XMARS) TO THE USER
pub fn update_state_on_claim(deps: DepsMut, env: Env,  user:Addr , prev_xmars_balance:Uint256)  -> StdResult<Response> {
 
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?; // Index is updated
    let mut user_info = USER_INFO.may_load(deps.storage, &user )?.unwrap_or_default();
    
    // QUERY:: Contract addresses
    let mars_contracts = vec![MarsContract::MarsToken, MarsContract::XMarsToken];
    let mut addresses_query = query_addresses(&deps.querier,config.address_provider.clone(), mars_contracts)?;
    let xmars_address = addresses_query.pop().unwrap();
    let mars_address = addresses_query.pop().unwrap();

    // Get XMARS Balance
    let cur_xmars_balance: cw20::BalanceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                                                                contract_addr: xmars_address.to_string(),
                                                                msg: to_binary(&mars::xmars_token::msg::QueryMsg::Balance {
                                                                    address: env.contract.address.to_string(),
                                                                }).unwrap(),
                                                            })).unwrap();
    // XMARS REWARDS CLAIMED (can be 0 in edge cases)                                                           
    let xmars_accured = Uint256::from(cur_xmars_balance.balance) - prev_xmars_balance;

    // UPDATE :: GLOBAL INDEX (XMARS rewards tracker)
    update_xmars_rewards_index(&mut state, xmars_accured);

    let mut total_mars_rewards = Uint256::zero();
    let mut total_xmars_rewards = Uint256::zero();

    // LOCKDROP :: LOOP OVER ALL LOCKUP POSITIONS TO CALCULATE THE LOCKDROP REWARD (if its not already claimed)
    if !user_info.lockdrop_claimed {
        let mut rewards = Uint256::zero();
        for lockup_id in &mut user_info.lockup_positions {
            let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.as_bytes())?.unwrap_or_default();
            rewards = calculate_lockdrop_reward(lockup_info.ust_locked , lockup_info.duration, &config, state.total_deposits_weight);
            lockup_info.lockdrop_reward = rewards;            
            total_mars_rewards += rewards;
            LOCKUP_INFO.save(deps.storage, lockup_id.as_bytes(), &lockup_info)?;
        }
        user_info.lockdrop_claimed = true;        
    }

    // TO BE CLAIMED :::: CALCULATE ACCRUED MARS AS DEPOSIT INCENTIVES
    compute_user_accrued_reward(&state, &mut user_info);        
    total_xmars_rewards = user_info.pending_reward;  
    user_info.pending_reward = Uint256::zero();

    // SAVE UPDATED STATES
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user.clone(), &user_info)?;

    let mut messages_ = vec![];
    // COSMOS MSG :: SEND MARS (LOCKDROP REWARD) IF > 0
    if total_mars_rewards > Uint256::zero() {
        let transfer_mars_msg = build_send_cw20_token_msg(user.clone(), mars_address, total_mars_rewards)?;
        messages_.push(transfer_mars_msg);
    }
    // COSMOS MSG :: SEND X-MARS (DEPOSIT INCENTIVES) IF > 0
    if total_xmars_rewards > Uint256::zero() {
        let transfer_xmars_msg = build_send_cw20_token_msg(user.clone(), xmars_address, total_xmars_rewards)?;
        messages_.push(transfer_xmars_msg);
    }

    Ok(Response::new()
        .add_messages(messages_)
        .add_attributes(vec![
            ("action", "lockdrop::CallbackMsg::ClaimRewards"),
            ("total_xmars_claimed", xmars_accured.to_string().as_str()),
            ("user", &user.to_string() ),
            ("mars_claimed", total_mars_rewards.to_string().as_str() ),
            ("xmars_claimed", total_xmars_rewards.to_string().as_str() ),
        ]))    
}



// CALLBACK :: CALLED BY try_unlock_position FUNCTION --> DELETES LOCKUP POSITION
pub fn try_dissolve_position( deps: DepsMut, _env: Env, user: Addr, duration: u64 ) -> StdResult<Response> { 

    // RETRIEVE :: User_Info and lockup position
    let mut user_info = USER_INFO.may_load(deps.storage, &user )?.unwrap_or_default();
    let lockup_id = user.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes() )?.unwrap_or_default();

    // UPDATE USER INFO
    user_info.total_ust_locked = user_info.total_ust_locked - lockup_info.ust_locked;

    // DISSOLVE LOCKUP POSITION
    lockup_info.ust_locked = Uint256::zero();
    remove_lockup_pos_from_user_info(&mut user_info, lockup_id.clone());

    USER_INFO.save(deps.storage, &user, &user_info)?;
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;

    Ok(Response::new()
        .add_attributes(vec![
            ("action", "lockdrop::Callback::DissolvePosition"),
            ("user", user.clone().as_str()),
            ("duration", duration.to_string().as_str())
        ]))
}


//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------


/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok (ConfigResponse {
        owner: config.owner.to_string(),
        address_provider: config.address_provider.to_string(),       
        ma_ust_token: config.ma_ust_token.to_string(),                 
        init_timestamp: config.init_timestamp,
        min_duration: config.min_lock_duration,
        max_duration: config.max_lock_duration,
        multiplier: config.weekly_multiplier,
        lockdrop_incentives: config.lockdrop_incentives
    })
}


/// @dev Returns the contract's Global State
pub fn query_state(deps: Deps) -> StdResult<GlobalStateResponse> {
    let state: State = STATE.load(deps.storage)?;
    Ok(GlobalStateResponse {
        final_ust_locked: state.final_ust_locked,
        final_maust_locked: state.final_maust_locked,
        total_ust_locked: state.total_ust_locked,
        total_maust_locked: state.total_maust_locked,
        global_reward_index: state.global_reward_index,
        total_deposits_weight: state.total_deposits_weight
    })
}


/// @dev Returns summarized details regarding the user
pub fn query_user_info(deps: Deps, user: String) -> StdResult<UserInfoResponse> {
    let user_address = deps.api.addr_validate(&user)?;
    let state: State = STATE.load(deps.storage)?;
    let user_info = USER_INFO.may_load(deps.storage, &user_address.clone() )?.unwrap_or_default();

    Ok(UserInfoResponse {
        total_ust_locked: user_info.total_ust_locked,
        total_maust_locked: calculate_user_ma_ust_share(user_info.total_ust_locked, state.final_ust_locked, state.final_maust_locked),
        lockup_position_ids: user_info.lockup_positions
    })
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info(deps: Deps, user: String, duration: u64) -> StdResult<LockUpInfoResponse> {
    let lockup_id = user.to_string() + &duration.to_string();
    query_lockup_info_with_id(deps, lockup_id)
}

/// @dev Returns summarized details regarding the user
pub fn query_lockup_info_with_id(deps: Deps, lockup_id: String) -> StdResult<LockUpInfoResponse> {
    let lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes() )?.unwrap_or_default();
    let state: State = STATE.load(deps.storage)?;

    let mut lockup_response = LockUpInfoResponse {
        duration : lockup_info.duration,
        ust_locked : lockup_info.ust_locked,         
        maust_balance : calculate_user_ma_ust_share(lockup_info.ust_locked, state.final_ust_locked, state.final_maust_locked),        
        lockdrop_reward : lockup_info.lockdrop_reward,   
        unlock_timestamp : lockup_info.unlock_timestamp
    };


    if lockup_response.lockdrop_reward  == Uint256::zero() {
        let config = CONFIG.load(deps.storage)?;
        lockup_response.lockdrop_reward = calculate_lockdrop_reward(lockup_response.ust_locked, lockup_response.duration, &config, state.total_deposits_weight );
    }

    Ok(lockup_response)
}





//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

/// true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config ) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

/// true if withdrawals are allowed
fn is_withdraw_open(current_timestamp: u64, config: &Config ) -> bool {
    let withdrawals_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (withdrawals_opened_till >= current_timestamp)
}

/// Returns the timestamp when the lockup will get unlocked
fn calculate_unlock_timestamp(config: &Config, duration:u64) -> u64 {
    config.init_timestamp + config.deposit_window + (duration*SECONDS_PER_WEEK)
}

// Calculate Lockdrop Reward
fn calculate_lockdrop_reward(deposited_ust:Uint256, duration: u64, config: &Config, total_deposits_weight: Uint256 ) -> Uint256 {
    if total_deposits_weight == Uint256::zero() {
        return Uint256::zero();
    }
    let amount_weight = calculate_weight(deposited_ust, duration, config.weekly_multiplier);
    config.lockdrop_incentives * Decimal256::from_ratio( amount_weight, total_deposits_weight )
}

// Returns effective weight for the amount to be used for calculating airdrop rewards
fn calculate_weight(amount:Uint256, duration: u64, weekly_multiplier:Decimal256) -> Uint256 {
    (amount * Uint256::from(duration)) * weekly_multiplier
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

// Accrue XMARS rewards by updating the reward index
fn update_xmars_rewards_index(state: &mut State, xmars_accured: Uint256) {
    let xmars_rewards_index = Decimal256::from_ratio(Uint256::from(xmars_accured) , Uint256::from(state.total_maust_locked) );
    state.global_reward_index = state.global_reward_index + xmars_rewards_index;
} 

// Accrue MARS reward for the user by updating the user reward index and adding rewards to the pending rewards
fn compute_user_accrued_reward(state: &State, user_info: &mut UserInfo) { 
    let user_maust_share = state.total_maust_locked * Decimal256::from_ratio(user_info.total_ust_locked, state.total_ust_locked);
    let pending_reward = (user_maust_share * state.global_reward_index) - (user_maust_share * user_info.reward_index);
    user_info.reward_index = state.global_reward_index;
    user_info.pending_reward += pending_reward;
}

// Returns User's maUST Token share :: Calculated as =  (User's deposited UST / Final UST deposited) * Final maUST Locked
fn calculate_user_ma_ust_share(lockup_ust_locked: Uint256, final_ust_locked: Uint256, final_maust_locked: Uint256 ) -> Uint256 {
    if final_ust_locked == Uint256::zero() {
        return Uint256::zero();
    }
    final_maust_locked * Decimal256::from_ratio(lockup_ust_locked, final_ust_locked)
}

// Returns true if the user_info stuct's lockup_positions vector contains the lockup_id
fn is_lockup_present_in_user_info(user_info: &UserInfo, lockup_id: String) ->bool {
    if user_info.lockup_positions.iter().any(|id| id == &lockup_id) {
        return true;
    }
    false
}


// REMOVE LOCKUP INFO FROM lockup_positions array IN USER INFO
fn remove_lockup_pos_from_user_info(user_info: &mut UserInfo, lockup_id: String) {
    let index = user_info.lockup_positions.iter().position(|x| *x == lockup_id).unwrap();
    user_info.lockup_positions.remove(index);    
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

// fn build_withdraw_from_redbank_msg(redbank_address: Addr, denom_stable: String, amount: Uint256) -> StdResult<CosmosMsg> {
//     Ok(CosmosMsg::Wasm(WasmMsg::Execute {
//         contract_addr: redbank_address.to_string(),
//         funds: vec![],
//         msg: to_binary(&mars::red_bank::msg::ExecuteMsg::Withdraw {
//             asset: mars::asset::Asset::Native { denom: denom_stable },
//             amount: Some(amount.into()),
//         })?,
//     }))
// }

fn build_claim_xmars_rewards(incentives_contract: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: incentives_contract.to_string(),
        funds: vec![],
        msg: to_binary(&mars::incentives::msg::ExecuteMsg::ClaimRewards {})?,
    }))
}
























































