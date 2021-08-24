#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_bignumber::{Decimal256, Uint256};

use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};

use crate::msg::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StakerInfoResponse, StateResponse,TimeResponse
};


use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use crate::state::{Config, CONFIG, State, STATE, StakerInfo , STAKER_INFO};

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(deps: DepsMut, env: Env, _info: MessageInfo, msg: InstantiateMsg, ) -> StdResult<Response> {

    let config = Config {
        mars_token: deps.api.addr_validate(&msg.mars_token)?,
        staking_token: deps.api.addr_validate(&msg.staking_token)?,
        init_timestamp: msg.init_timestamp,
        till_timestamp: msg.till_timestamp,
        cycle_duration: msg.cycle_duration,
        reward_increase: msg.reward_increase,
    };

    config.validate()?;
    CONFIG.save( deps.storage, &config)?;

    STATE.save(
        deps.storage,
        &State {
            cycle_init_timestamp: msg.init_timestamp,
            cycle_rewards: msg.cycle_rewards,
            last_distributed: env.block.time.seconds(),
            total_bond_amount: Uint256::zero(),
            global_reward_index: Decimal256::zero(),
        }
    )?;

    Ok(Response::default())
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond { amount } => unbond(deps, env, info, amount),
        ExecuteMsg::Claim {} => try_claim(deps, env, info),
    }
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State { timestamp } => to_binary(&query_state(deps, _env, timestamp)?),
        QueryMsg::StakerInfo { staker, timestamp } => to_binary(&query_staker_info(deps,_env, staker, timestamp)?),
        QueryMsg::Timestamp { } => to_binary(&query_timestamp( _env)?),
    }
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------


pub fn receive_cw20(deps: DepsMut, env: Env, info: MessageInfo, cw20_msg: Cw20ReceiveMsg) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => {
            // only staking token contract can execute this message
            if config.staking_token.to_string() != info.sender.as_str() {
                return Err(StdError::generic_err("unauthorized"));
            }

            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
            bond(deps, env, cw20_sender, cw20_msg.amount.into())
        }
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}



pub fn bond(deps: DepsMut, env: Env, sender_addr: Addr, amount: Uint256) -> StdResult<Response> {

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO.may_load(deps.storage, &sender_addr)?.unwrap_or_default();

    compute_reward( &config, &mut state, env.block.time.seconds() );      // Compute global reward
    compute_staker_reward(&state, &mut staker_info)?;                       // Compute staker reward
    increase_bond_amount(&mut state, &mut staker_info, amount);             // Increase bond_amount

    // Store updated state with staker's staker_info
    STAKER_INFO.save( deps.storage, &sender_addr, &staker_info)?;
    STATE.save( deps.storage, &state )?;

    Ok(Response::new().add_attributes(vec![
        ("action", "bond"),
        ("owner", sender_addr.as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}



pub fn unbond(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint256) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    let sender_addr = info.sender;

    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo = STAKER_INFO.may_load(deps.storage, &sender_addr)?.unwrap_or_default();

    if staker_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }
    
    compute_reward(&config, &mut state, env.block.time.seconds());  // Compute global reward & staker reward
    compute_staker_reward(&state, &mut staker_info)?;                   // Compute staker reward
    
    let reward_amount = staker_info.pending_reward;
    staker_info.pending_reward = Uint256::zero();

    decrease_bond_amount(&mut state, &mut staker_info, amount)?;        // Decrease bond_amount

    // Store or remove Staker info, depends on the left bond amount
    if staker_info.bond_amount.is_zero() {
        STAKER_INFO.remove( deps.storage, &sender_addr);
    } else {
        STAKER_INFO.save( deps.storage, &sender_addr, &staker_info)?;
    }

    STATE.save( deps.storage, &state )?;                     // Store updated state

    // UNBOND STAKED TOKEN , TRANSFER $MARS
    Ok(Response::new()    
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.staking_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender_addr.to_string(),
                amount: amount.into(),
            })?,
            funds: vec![],
        })])
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.mars_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender_addr.to_string(),
                amount: reward_amount.into(),
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", "unbond"),
            ("owner", sender_addr.as_str()),
            ("amount", amount.to_string().as_str()),
            ("rewards", reward_amount.to_string().as_str()),
        ])
    )
}



// withdraw rewards to executor
pub fn try_claim(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let sender_addr = info.sender;

    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO.may_load(deps.storage, &sender_addr)?.unwrap_or_default();

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info)?;

    let amount = staker_info.pending_reward;
    staker_info.pending_reward = Uint256::zero();

    STAKER_INFO.save( deps.storage, &sender_addr, &staker_info)?;    // Update Staker Info
    STATE.save( deps.storage, &state )?;                               // Store updated state

    Ok(Response::new()
        .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.mars_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: sender_addr.to_string(),
                amount: amount.into(),
            })?,
            funds: vec![],
        })])
        .add_attributes(vec![
            ("action", "claim_rewards"),
            ("owner", sender_addr.as_str()),
            ("rewards", amount.to_string().as_str()),
        ])
    )
}



fn increase_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint256) {
    state.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

fn decrease_bond_amount(state: &mut State,staker_info: &mut StakerInfo,amount: Uint256,) -> StdResult<()> {
    state.total_bond_amount = state.total_bond_amount - amount;
    staker_info.bond_amount = staker_info.bond_amount - amount;
    Ok(())
}

// compute distributed rewards and update global reward index
fn compute_reward(config: &Config, state: &mut State, cur_timestamp: u64) {
    if state.total_bond_amount.is_zero() || config.init_timestamp > cur_timestamp {
        state.last_distributed = cur_timestamp;
        return;
    }

    let mut rewards_to_distribute = Decimal256::zero();
    let next_cycle_init_timestamp = state.cycle_init_timestamp + config.cycle_duration;

    // Next Cycle has begun
    if next_cycle_init_timestamp <= cur_timestamp {    
        // Rewards to be distributed from previous cycle
        rewards_to_distribute = Decimal256::from_uint256(next_cycle_init_timestamp - state.last_distributed) * Decimal256::from_ratio(state.cycle_rewards, config.cycle_duration);
        // Update Current Cycle       
        state.cycle_init_timestamp = next_cycle_init_timestamp;                                                   
        // Update rewards distributed per cycle
        state.cycle_rewards = state.cycle_rewards + (state.cycle_rewards * config.reward_increase );  
        // Rewards to be distributed from current cycle
        rewards_to_distribute = rewards_to_distribute + Decimal256::from_uint256(cur_timestamp - next_cycle_init_timestamp) * Decimal256::from_ratio(state.cycle_rewards, config.cycle_duration);
    }
    // Current Cycle in progress
    else {
        rewards_to_distribute = Decimal256::from_uint256(cur_timestamp - state.last_distributed) * Decimal256::from_ratio(state.cycle_rewards, config.cycle_duration);
    }

    state.last_distributed = cur_timestamp;
    state.global_reward_index = state.global_reward_index + (rewards_to_distribute / Decimal256::from_uint256(state.total_bond_amount));
}


// withdraw reward to pending reward
fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.global_reward_index) - (staker_info.bond_amount * staker_info.reward_index);
    staker_info.reward_index = state.global_reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}


//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------


pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let resp = ConfigResponse {
        mars_token: config.mars_token.to_string(),
        staking_token: config.staking_token.to_string(),
        init_timestamp: config.init_timestamp,
        till_timestamp: config.till_timestamp,
        cycle_duration: config.cycle_duration,
        reward_increase: config.reward_increase,
    };

    Ok(resp)
}

pub fn query_state(deps: Deps, env:Env, timestamp: Option<u64>) -> StdResult<StateResponse> {
    let mut state: State = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    match timestamp {
        Some(timestamp) => {
            compute_reward(&config, &mut state, timestamp);
        }
        None => {
            compute_reward(&config, &mut state, env.block.time.seconds());
        }
    }

    // if let Some(timestamp) = timestamp {
    //     compute_reward(&config, &mut state, timestamp);
    // }

    Ok(StateResponse {
        cycle_init_timestamp: state.cycle_init_timestamp,
        cycle_rewards: state.cycle_rewards,
        last_distributed: state.last_distributed,
        total_bond_amount: state.total_bond_amount,
        global_reward_index: state.global_reward_index,
    })
}

pub fn query_staker_info( deps: Deps, env:Env, staker: String, timestamp: Option<u64>) -> StdResult<StakerInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO.may_load(deps.storage, &deps.api.addr_validate(&staker)?)?.unwrap_or_default();

    match timestamp {
        Some(timestamp) => {
            compute_reward(&config, &mut state, timestamp);
            compute_staker_reward(&state, &mut staker_info)?;    
        }
        None => {
            compute_reward(&config, &mut state, env.block.time.seconds());
            compute_staker_reward(&state, &mut staker_info)?;    
        }
    }

    Ok(StakerInfoResponse {
        staker,
        reward_index: staker_info.reward_index,
        bond_amount: staker_info.bond_amount,
        pending_reward: staker_info.pending_reward,
    })
}


pub fn query_timestamp( env: Env) -> StdResult<TimeResponse> {
    Ok(TimeResponse { timestamp: env.block.time.seconds() })
}
