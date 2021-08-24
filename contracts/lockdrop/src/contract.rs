#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, Uint128, QuerierWrapper,CosmosMsg, BankMsg, QueryRequest,WasmQuery, Addr, Coin, DepsMut, Env, MessageInfo, WasmMsg, Response, StdResult, StdError};
use cosmwasm_bignumber::{Decimal256, Uint256};

use crate::msg::{CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CONFIG, State, STATE, UserInfo, USER_INFO, LockupInfo, LOCKUP_INFO};

const SECONDS_PER_YEAR: u64 = 31536000u64;

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate( deps: DepsMut,_env: Env,info: MessageInfo,msg: InstantiateMsg ) -> StdResult<Response> {

    let config = Config {
        red_bank: deps.api.addr_validate(&msg.red_bank)?,
        mars_token: deps.api.addr_validate(&msg.mars_token)?,
        maUST_token: deps.api.addr_validate(&msg.maUST_token)?,
        incentives_contract: deps.api.addr_validate(&msg.incentives_contract)?,
        init_timestamp: msg.init_timestamp,
        min_lock_duration: msg.min_duration,
        max_lock_duration: msg.max_duration,
        borrow_ltv: msg.borrow_ltv,
        denom: msg.denom,
        weekly_multiplier: msg.multiplier
    };

    let state = State {
        owner: deps.api.addr_validate(&msg.owner)?,
        total_UST_locked: Uint256::zero(),
        total_maUST_locked: Uint256::zero(),
        total_UST_borrowed: Uint256::zero(),
        global_interest_index: Decimal256::zero(),
        global_reward_index: Decimal256::zero(),
        lockdrop_rewards: Uint256::zero(),
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
        ExecuteMsg::BorrowUST { amount} => try_borrow_UST(deps, _env, info, amount ),
        ExecuteMsg::RepayUST { } => try_repay_UST(deps,_env, info)
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

// USER SENDS UST --> CONTRACT DEPOSITS IT INTO RED BANK --> USER'S LOCKUP POSITION IS UPDATED
pub fn try_lock_UST( deps: DepsMut, env: Env, info: MessageInfo, duration: u64 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

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

    // maUST minted against the deposit
    let minted_ma_ust = calculate_minted_maUST(&deps.querier, config.denom.clone(), config.red_bank.clone()  , deposit_amount, env.block.time.seconds())?;
    
    // STATE :: UPDATE
    state.total_UST_locked += deposit_amount;
    state.total_maUST_locked +=  minted_ma_ust;

    // LOCKUP INFO :: RETRIEVE --> UPDATE --> SAVE
    let lockup_id = depositor_address.clone().to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.clone().as_bytes() )?.unwrap_or_default();
    lockup_info.user = depositor_address.clone();
    lockup_info.duration = duration;
    lockup_info.ma_UST_locked += minted_ma_ust;
    lockup_info.unlock_timestamp = config.init_timestamp + (duration*(86400 as u64));

    // USER INFO :: RETRIEVE --> UPDATE --> SAVE
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address.clone() )?.unwrap_or_default();
    user_info.total_ust_locked += deposit_amount;
    user_info.total_ma_UST_locked += minted_ma_ust;
    user_info.lockup_positions.push(lockup_id.clone() );

    STATE.save(deps.storage, &state);
    LOCKUP_INFO.save(deps.storage, lockup_id.clone().as_bytes(), &lockup_info)?;
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    // Red Bank Deposit MSG
    let redbank_deposit_msg = build_deposit_into_redbank_msg(config.red_bank, config.denom.clone(), deposit_amount)?;

    Ok(Response::new()
        .add_messages(vec![redbank_deposit_msg])
        .add_attributes(vec![
            ("action", "ust_locked"),
            ("user", &depositor_address.to_string()),
            ("duration", duration.to_string().as_str()),
            ("ust_amount", deposit_amount.to_string().as_str()),
            ("maUST_amount", minted_ma_ust.to_string().as_str()),
        ]))
}


// USER UNLOCKS UST --> CONTRACT WITHDRAWS FROM RED BANK --> REWARDS AND UST IS RETURNED TO THE USER
pub fn try_unlock_UST( deps: DepsMut, env: Env, info: MessageInfo, duration: u64 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();
    let current_timestamp = env.block.time.seconds();

    // USER INFO :: RETRIEVE --> UPDATE  
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address )?.unwrap_or_default();

    // LOCKUP INFO :: RETRIEVE --> DELETE 
    let lockup_id = depositor_address.to_string() + &duration.to_string();
    let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockup_id.as_bytes() )?.unwrap_or_default();

    // CHECK :: IS VALID LOCKUP
    if lockup_info.ma_UST_locked == Uint256::zero() {
        return Err(StdError::generic_err("No lockup to unlock"));
    }    

    // CHECK :: LOCKUP CAN BE UNLOCKED 
    if lockup_info.unlock_timestamp > current_timestamp.clone() {
        let time_remaining = lockup_info.unlock_timestamp - current_timestamp.clone();
        return Err(StdError::generic_err(format!("{} seconds to Unlock",time_remaining)));
    }

    // CALCULATE :: UST BEING UNLOCKED
    let ust_being_unlocked = calculate_maUST_value(&deps.querier, config.red_bank.clone() , config.denom.clone(), lockup_info.ma_UST_locked, current_timestamp.clone());
    user_info.total_ust_locked = user_info.total_ust_locked - ust_being_unlocked;

    // CHECK :: BORROW BALANCE SHOULDN'T EXCEED MAX. ALLOWED BORROW LIMIT
    if user_info.ust_borrowed > (user_info.total_ust_locked * config.borrow_ltv) {
        let max_borrow = user_info.total_ust_locked * config.borrow_ltv;
        return Err(StdError::generic_err(format!("UST borrow balance exceeds max. allowed borrow limit. Borrowed UST = {}. New max. borrow limit = {} UST",user_info.ust_borrowed, max_borrow)));
    }

    let mut rewards = Uint256::zero();

    // TO BE CLAIMED ?? :::: CALCULATE LOCKDROP REWARD
    if lockup_info.lockdrop_reward == Uint256::zero() {
        rewards = calculate_lockdrop_reward(ust_being_unlocked, lockup_info.duration, state.lockdrop_rewards, config.weekly_multiplier);
        lockup_info.lockdrop_reward = rewards;
    }

    compute_accrued_reward(&deps.querier, env, &config, &mut state);            // Compute global reward 
    compute_staker_accrued_reward(state, &mut lockup_info);           // Compute depositor reward

    rewards += lockup_info.pending_reward;

    // UPDATE STATE
    state.total_UST_locked = state.total_UST_locked - ust_being_unlocked;
    state.total_maUST_locked = state.total_maUST_locked.clone() - lockup_info.ma_UST_locked;

    // UPDATE USER INFO
    user_info.total_ma_UST_locked = user_info.total_ma_UST_locked - lockup_info.ma_UST_locked;
    user_info.total_ust_locked = calculate_maUST_value(&deps.querier, config.red_bank.clone() , config.denom.clone(), user_info.total_ma_UST_locked, current_timestamp.clone());
    // REMOVE LOCKUP INFO FROM lockup_positions array IN USER INFO
    let index = user_info.lockup_positions.iter().position(|x| *x == lockup_id).unwrap();
    user_info.lockup_positions.remove(index);

    STATE.save(deps.storage, &state);
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    // REMOVE LOCKUP DETAILS
    LOCKUP_INFO.remove(deps.storage, lockup_id.as_bytes());

    // MSG :: Transfer $MARS Rewards
    let transfer_mars_msg = build_send_cw20_token_msg(depositor_address.clone(), config.mars_token, rewards)?;
    // MSG :: Red Bank Withdrawal
    let redbank_withdraw_msg = build_withdraw_from_redbank_msg(config.red_bank, config.denom.clone(), lockup_info.ma_UST_locked)?;
    // MSG :: Transfer UST
    let transfer_ust_msg = build_send_native_asset_msg(depositor_address.clone(), &config.denom.clone(), ust_being_unlocked)?;    

    Ok(Response::new()
        .add_messages(vec![transfer_mars_msg,redbank_withdraw_msg, transfer_ust_msg])
        .add_attributes(vec![
            ("action", "ust_locked"),
            ("owner", info.sender.as_str()),
            ("duration", duration.to_string().as_str()),
            ("amount", ust_being_unlocked.to_string().as_str()),
            ("rewards", rewards.to_string().as_str())
        ]))
}




// USER UNLOCKS UST --> CONTRACT WITHDRAWS FROM RED BANK --> REWARDS AND UST IS RETURNED TO THE USER
pub fn try_claim( deps: DepsMut, env: Env, info: MessageInfo ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();
    let current_timestamp = env.block.time.seconds();

    // CHECK :: Lockdrop deposit window closed
    if config.init_timestamp > current_timestamp.clone() {
        return Err(StdError::generic_err("Claim not allowed during deposit window"));
    }    

    // USER INFO :: RETRIEVE --> UPDATE --> SAVE
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address )?.unwrap_or_default();

    // CHECK :: User has valid locked deposit positions
    if user_info.total_ma_UST_locked == Uint256::zero() {
        return Err(StdError::generic_err("No valid lockup found"));
    }

    // COMPUTE :: GLOBALLY ACCRUED DEPOSIT INCENTIVES
    compute_accrued_reward(&deps.querier, env, &config, &mut state);           
    
    // CALCULATE :: Total Claimiable MARS Rewards
    let mut total_rewards = Uint256::zero();
    let total_lockdrop_rewards = state.lockdrop_rewards;

    // LOOP OVER ALL LOCKUP POSITIONS :: UPDATE EACH POSITION
    for lockupId in &mut user_info.lockup_positions {

        let mut rewards = Uint256::zero();
        let mut lockup_info = LOCKUP_INFO.may_load(deps.storage, lockupId.as_bytes())?.unwrap_or_default();

        // TO BE CLAIMED :::: CALCULATE LOCKDROP REWARD
        if lockup_info.lockdrop_reward == Uint256::zero() {
            let locked_maust_worth = calculate_maUST_value(&deps.querier, config.red_bank.clone() , config.denom.clone(), lockup_info.ma_UST_locked, current_timestamp.clone());
            rewards = calculate_lockdrop_reward(locked_maust_worth, lockup_info.duration, total_lockdrop_rewards.clone(), config.weekly_multiplier);
            lockup_info.lockdrop_reward = rewards;
        }
        
        // TO BE CLAIMED :::: CALCULATE ACCRUED DEPOSIT INCENTIVES
        compute_staker_accrued_reward(state, &mut lockup_info);        
        rewards += lockup_info.pending_reward;  
        lockup_info.pending_reward = Uint256::zero();

        // TO BE CLAIMED :::: ADD TO TOTAL REWARDS ACCRUED
        total_rewards += rewards;

        // LOCKUP INFO :: SAVE UPDATED STATE
        LOCKUP_INFO.save(deps.storage, lockupId.as_bytes(), &lockup_info);
    }

    // MSG :: Transfer $MARS Rewards
    let transfer_mars_msg = build_send_cw20_token_msg(depositor_address.clone(), config.mars_token, total_rewards)?;

    Ok(Response::new()
        .add_messages(vec![transfer_mars_msg])
        .add_attributes(vec![
            ("action", "claim_reward"),
            ("owner", depositor_address.clone().as_str()),
            ("amount", total_rewards.to_string().as_str()),
        ]))    
}




// USER BORROWS UST FROM RED BANK 
pub fn try_borrow_UST( deps: DepsMut, env: Env, info: MessageInfo, amount: Uint256 ) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();

    // CHECK :: Lockdrop deposit window closed
    if config.init_timestamp > env.block.time.seconds() {
        return Err(StdError::generic_err("Lockdrop window is not closed yet"));
    }

    // USER INFO :: RETRIEVE --> UPDATE --> SAVE
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address)?.unwrap_or_default();
    user_info.total_ust_locked = calculate_maUST_value(&deps.querier, config.red_bank.clone() , config.denom.clone(), user_info.total_ma_UST_locked, env.block.time.seconds());
    let max_borrow_allowed = calculate_max_borrow_allowed(user_info.total_ust_locked, config.borrow_ltv);

    // CHECK :: Check if UST amount can be borrowed
    if (amount + user_info.ust_borrowed) > max_borrow_allowed  {
        return Err(StdError::generic_err(format!("Max borrow allowed : {} UST. Already borrowed : {} UST",max_borrow_allowed, user_info.ust_borrowed)));
    }

    state.total_UST_borrowed += amount;

    // COMPUTE :: Global & User's interest accrued
    compute_accrued_interest(&deps.querier, env, &config, &mut state);                        
    compute_staker_accrued_interest(state, &mut user_info);                    

    user_info.ust_borrowed += amount;

    STATE.save(deps.storage, &state.clone() );
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    let borrow_msg = build_borrow_from_redbank_msg(config.red_bank.clone(), config.denom.clone(), amount)?;
    
    Ok(Response::new()
        .add_messages(vec![borrow_msg])
        .add_attributes(vec![
            ("action", "ust_borrowed"),
            ("borrower", depositor_address.as_str()),
            ("amount", &amount.to_string() ),
        ]))
}



// USER BORROWS UST FROM RED BANK 
pub fn try_repay_UST( deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let depositor_address = info.sender.clone();
    let mut user_info = USER_INFO.may_load(deps.storage, &depositor_address)?.unwrap_or_default();

    // Get UST amount to be repaid
    let repay_amount = get_denom_amount_from_coins(&info.funds, &config.denom);
    
    // CHECK :: Valid repay amount
    if repay_amount == Uint256::zero() {
        return Err(StdError::generic_err("Repay amount cannot be zero"));
    }

    // CHECK :: Check if debt to be repaid
    if user_info.ust_borrowed == Uint256::zero()  {
        return Err(StdError::generic_err("No debt to repay"));
    }
    
    // COMPUTE :: Global & User's interest accrued
    compute_accrued_interest(&deps.querier, env, &config, &mut state);                         
    compute_staker_accrued_interest(state, &mut user_info);               

    // CALCULATE UST TO REPAY AND AMOUNT TO BE SENT BACK (if any)
    let amount_to_repay = std::cmp::min(repay_amount, user_info.ust_borrowed);
    let mut payback = Uint256::zero();

    // CALCULATE :: REPAY AND PAYBACK
    if amount_to_repay <= user_info.ust_borrowed {
        user_info.ust_borrowed = user_info.ust_borrowed - amount_to_repay;
    } else {
        payback = amount_to_repay - user_info.ust_borrowed;
        user_info.ust_borrowed = Uint256::zero();
    }

    state.total_UST_borrowed = state.total_UST_borrowed - amount_to_repay;

    STATE.save(deps.storage, &state);
    USER_INFO.save(deps.storage, &depositor_address.clone(), &user_info)?;

    // MSGS :: REPAY AND PAYBACK
    let mut msgs = vec![];
    let repay_msg = build_repay_to_redbank_msg(config.red_bank, config.denom.clone(), amount_to_repay)?;
    msgs.push(repay_msg);
    if payback > Uint256::zero() {
        msgs.push( build_send_native_asset_msg(depositor_address.clone(), &config.denom.clone(), payback)? );
    }
    
    Ok(Response::new()
        .add_messages(msgs)
        .add_attributes(vec![
            ("action", "ust_repaid"),
            ("borrower", depositor_address.clone().as_str() ),
            ("amount", &amount_to_repay.to_string() ),
            ("payback", &payback.to_string() ),
    ]))
}






// HELPERS

// Calculate Lockdrop Reward
fn calculate_lockdrop_reward(deposit_amount:Uint256, duration: u64, total_rewards: Uint256, weekly_multiplier:Decimal256) -> Uint256 {
    let _multiplier = Decimal256::from_ratio(duration, 7 as u64) * weekly_multiplier;
    Decimal256::from_uint256(deposit_amount) * _multiplier * total_rewards
}

// Return max. UST amount that can be borrowed by a user
fn calculate_max_borrow_allowed(ust_locked: Uint256, borrow_ltv: Decimal256) -> Uint256 {
    ust_locked * borrow_ltv
}

// native coins
fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

// INTEREST COMPUTATION

// Accrue interest globally by updating the Global Interest Index 
fn compute_accrued_interest(querier: &QuerierWrapper, env:Env, config: &Config, state: &mut State) {
    // Query Total UST Debt of the lockdrop contract
    let debts_: mars::red_bank::msg::DebtResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.red_bank.to_string(),
        msg: to_binary(&mars::red_bank::msg::QueryMsg::Debt { address: env.contract.address.to_string() }).unwrap(),
    })).unwrap();   

    let debt:&mars::red_bank::msg::DebtInfo = debts_.debts.iter().find(|debt| debt.denom == config.denom ).unwrap();
    let ust_debt = debt.amount;
    
    state.global_interest_index = Decimal256::from_ratio(ust_debt, state.total_UST_borrowed);
} 

// Accrue interest for a user by updating the Interest Index 
fn compute_staker_accrued_interest(state: State, user_info: &mut UserInfo) {
    let interest_accrued = (user_info.ust_borrowed * state.global_interest_index) - (user_info.ust_borrowed * user_info.interest_index);
    user_info.ust_borrowed += interest_accrued;
    user_info.interest_index = state.global_interest_index;
}

// MARS REWARDS COMPUTATION

// Accrue MARS reward by updating the reward index
fn compute_accrued_reward(querier: &QuerierWrapper, env:Env, config: &Config, state: &mut State) {

    // Get MARS reward accrued by the contract
    let accrued_reward: Uint128 = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.incentives_contract.to_string(),
        msg: to_binary(&mars::incentives::msg::QueryMsg::UserUnclaimedRewards {
            user_address: env.contract.address.to_string(),
        }).unwrap(),
    })).unwrap();   
    
    // Get maUST Balance
    let maUST_balance: cw20::BalanceResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: config.maUST_token.to_string(),
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





// RETURNS maUST THAT WILL BE MINTED
pub fn calculate_minted_maUST(querier: &QuerierWrapper, denom: String, red_bank_: Addr, deposit_amount: Uint256, current_timestamp: u64 ) -> StdResult<Uint256> {
    

    let query: mars::red_bank::msg::MarketResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: red_bank_.to_string(),
        msg: to_binary(&mars::red_bank::msg::QueryMsg::Market { asset: mars::asset::Asset::Native { denom: denom } }).unwrap(),
    })).unwrap();   

    let interests_last_updated = query.interests_last_updated;
    let liquidity_rate = query.liquidity_rate;
    let liquidity_index =  query.liquidity_index;
     
    let mut updated_liquidity_index = liquidity_index;
     
    if interests_last_updated < current_timestamp {
        let time_elapsed = current_timestamp - interests_last_updated;
     
            if liquidity_rate > Decimal256::zero() {
                let applied_interest_rate = calculate_applied_linear_interest_rate( liquidity_index, liquidity_rate, time_elapsed );
                updated_liquidity_index = applied_interest_rate;
            }
    }
     
        let mint_amount = deposit_amount / updated_liquidity_index;
        return Ok(mint_amount);
}

fn calculate_applied_linear_interest_rate( index: Decimal256, rate: Decimal256, time_elapsed: u64, ) -> Decimal256 {
    let rate_factor = rate * Decimal256::from_uint256(time_elapsed) / Decimal256::from_uint256(SECONDS_PER_YEAR);
    index * (Decimal256::one() + rate_factor)
}


pub fn calculate_maUST_value(querier: &QuerierWrapper, red_bank_: Addr, denom:String, deposited_maust: Uint256, block_time: u64 ) -> Uint256 {
    let withdrawer_balance_scaled = deposited_maust;

    // Get Market state
    let query: mars::red_bank::msg::MarketResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: red_bank_.to_string(),
        msg: to_binary(&mars::red_bank::msg::QueryMsg::Market { asset: mars::asset::Asset::Native { denom: denom } }).unwrap(),
    })).unwrap();   

    let interests_last_updated = query.interests_last_updated;
    let liquidity_rate = query.liquidity_rate;
    let liquidity_index =  query.liquidity_index;
     
    let mut updated_liquidity_index = liquidity_index;

    if interests_last_updated < block_time {
        let time_elapsed = block_time - interests_last_updated;
     
            if liquidity_rate > Decimal256::zero() {
                let applied_interest_rate = calculate_applied_linear_interest_rate( liquidity_index, liquidity_rate, time_elapsed );
                updated_liquidity_index = applied_interest_rate;
            }
    }

    withdrawer_balance_scaled * updated_liquidity_index
}

     
     

    

// COSMOS_MSGs     

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
 
fn build_borrow_from_redbank_msg(redbank_address: Addr, denom_: String, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: redbank_address.to_string(),
        funds: vec![],
        msg: to_binary(&mars::red_bank::msg::ExecuteMsg::Borrow {
            asset: mars::asset::Asset::Native { denom: denom_ },
            amount: amount.into()
        })?,
    }))
}

fn build_repay_to_redbank_msg(redbank_address: Addr, denom_: String, amount: Uint256) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: redbank_address.to_string(),
        funds: vec![ Coin { denom: denom_.clone(), amount: amount.into() } ],
        msg: to_binary(&mars::red_bank::msg::ExecuteMsg::RepayNative {
            denom: denom_,
        })?,
    }))
}























































