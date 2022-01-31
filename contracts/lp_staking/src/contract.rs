use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Api, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use mars_periphery::lp_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StakerInfoResponse, StateResponse, TimeResponse, UpdateConfigMsg,
};

use crate::state::{Config, StakerInfo, State, CONFIG, STAKER_INFO, STATE};

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    if msg.init_timestamp < env.block.time.seconds() || msg.till_timestamp < msg.init_timestamp {
        return Err(StdError::generic_err("Invalid timestamps"));
    }

    if msg.cycle_duration == 0u64 {
        return Err(StdError::generic_err("Invalid cycle duration"));
    }

    let config = Config {
        owner: deps.api.addr_validate(&msg.owner.unwrap())?,
        mars_token: deps.api.addr_validate(&msg.mars_token)?,
        staking_token: option_string_to_addr(deps.api, msg.staking_token, zero_address())?,
        init_timestamp: msg.init_timestamp,
        till_timestamp: msg.till_timestamp,
        cycle_duration: msg.cycle_duration,
        reward_increase: msg.reward_increase.unwrap_or_else(Decimal::zero),
    };

    config.validate()?;
    CONFIG.save(deps.storage, &config)?;

    STATE.save(
        deps.storage,
        &State {
            current_cycle: 0u64,
            current_cycle_rewards: msg.cycle_rewards.unwrap_or_else(Uint128::zero),
            last_distributed: msg.init_timestamp,
            total_bond_amount: Uint128::zero(),
            global_reward_index: Decimal::zero(),
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => update_config(deps, env, info, new_config),
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::Unbond {
            amount,
            withdraw_pending_reward,
        } => unbond(deps, env, info, amount, withdraw_pending_reward),
        ExecuteMsg::Claim {} => try_claim(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State { timestamp } => to_binary(&query_state(deps, _env, timestamp)?),
        QueryMsg::StakerInfo { staker, timestamp } => {
            to_binary(&query_staker_info(deps, _env, staker, timestamp)?)
        }
        QueryMsg::Timestamp {} => to_binary(&query_timestamp(_env)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Err(StdError::generic_err("unimplemented"))
}
//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

/// Only MARS-UST LP Token can be sent to this contract via the Cw20ReceiveMsg Hook
/// @dev Increases user's staked LP Token balance via the Bond Function
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;

    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Bond {}) => {
            // only staking token contract can execute this message
            if config.staking_token != info.sender.as_str() {
                return Err(StdError::generic_err("unauthorized"));
            }
            let cw20_sender = deps.api.addr_validate(&cw20_msg.sender)?;
            bond(deps, env, cw20_sender, cw20_msg.amount)
        }
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}

/// @dev Called by receive_cw20(). Increases user's staked LP Token balance
/// @params sender_addr : User Address who sent the LP Tokens
/// @params amount : Number of LP Tokens transferred to the contract
pub fn bond(deps: DepsMut, env: Env, sender_addr: Addr, amount: Uint128) -> StdResult<Response> {
    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO
        .may_load(deps.storage, &sender_addr)?
        .unwrap_or_default();

    compute_reward(&config, &mut state, env.block.time.seconds()); // Compute global reward
    compute_staker_reward(&state, &mut staker_info)?; // Compute staker reward
    increase_bond_amount(&mut state, &mut staker_info, amount); // Increase bond_amount

    // Store updated state with staker's staker_info
    STAKER_INFO.save(deps.storage, &sender_addr, &staker_info)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "Staking::ExecuteMsg::Bond"),
        ("user", sender_addr.as_str()),
        ("amount", amount.to_string().as_str()),
        ("total_bonded", staker_info.bond_amount.to_string().as_str()),
    ]))
}

/// @dev Only owner can call this function. Updates the config
/// @dev init_timestamp : can only be updated before it gets elapsed
/// @dev till_timestamp : can only be updated before it gets elapsed
/// @params new_config : New config object
pub fn update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // ONLY OWNER CAN UPDATE CONFIG
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // ACCURE CURRENT REWARDS IN-CASE `reward_increase` / `current_cycle_rewards` ARE UPDATED
    compute_reward(&config, &mut state, env.block.time.seconds()); // Compute global reward & staker reward

    // UPDATE :: ADDRESSES IF PROVIDED
    config.staking_token =
        option_string_to_addr(deps.api, new_config.staking_token, config.staking_token)?;
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.owner)?;

    // UPDATE :: VALUES IF PROVIDED
    if let Some(new_increase_ratio) = new_config.reward_increase {
        if new_increase_ratio < Decimal::one() {
            config.reward_increase = new_increase_ratio;
        } else {
            return Err(StdError::generic_err("Invalid reward increase ratio"));
        }
    }

    state.current_cycle_rewards = new_config
        .cycle_rewards
        .unwrap_or(state.current_cycle_rewards);

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attribute("action", "staking::ExecuteMsg::UpdateConfig"))
}

/// @dev Reduces user's staked position. MARS Rewards are transferred along-with unstaked LP Tokens
/// @params amount :  Number of LP Tokens transferred to be unstaked
pub fn unbond(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    withdraw_pending_reward: Option<bool>,
) -> StdResult<Response> {
    let sender_addr = info.sender;
    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info: StakerInfo = STAKER_INFO
        .may_load(deps.storage, &sender_addr)?
        .unwrap_or_default();

    if staker_info.bond_amount < amount {
        return Err(StdError::generic_err("Cannot unbond more than bond amount"));
    }

    compute_reward(&config, &mut state, env.block.time.seconds()); // Compute global reward & staker reward
    compute_staker_reward(&state, &mut staker_info)?; // Compute staker reward
    decrease_bond_amount(&mut state, &mut staker_info, amount); // Decrease bond_amount
    let mut messages = vec![];
    let mut claimed_rewards = Uint128::zero();

    if let Some(withdraw_pending_reward) = withdraw_pending_reward {
        if withdraw_pending_reward {
            claimed_rewards = staker_info.pending_reward;
            if claimed_rewards > Uint128::zero() {
                staker_info.pending_reward = Uint128::zero();
                messages.push(build_send_cw20_token_msg(
                    sender_addr.clone(),
                    config.mars_token,
                    claimed_rewards,
                )?);
            }
        }
    }

    // Store Staker info, depends on the left bond amount
    STAKER_INFO.save(deps.storage, &sender_addr, &staker_info)?;
    STATE.save(deps.storage, &state)?;

    messages.push(build_send_cw20_token_msg(
        sender_addr.clone(),
        config.staking_token,
        amount,
    )?);

    // UNBOND STAKED TOKEN , TRANSFER $MARS
    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "Staking::ExecuteMsg::Unbond"),
        ("user", sender_addr.as_str()),
        ("amount", amount.to_string().as_str()),
        ("total_bonded", staker_info.bond_amount.to_string().as_str()),
        ("claimed_rewards", claimed_rewards.to_string().as_str()),
    ]))
}

/// @dev Function to claim accrued MARS Rewards
pub fn try_claim(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let sender_addr = info.sender;
    let config: Config = CONFIG.load(deps.storage)?;
    let mut state: State = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO
        .may_load(deps.storage, &sender_addr)?
        .unwrap_or_default();

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());
    compute_staker_reward(&state, &mut staker_info)?;

    let accrued_rewards = staker_info.pending_reward;
    staker_info.pending_reward = Uint128::zero();

    STAKER_INFO.save(deps.storage, &sender_addr, &staker_info)?; // Update Staker Info
    STATE.save(deps.storage, &state)?; // Store updated state

    let mut messages = vec![];

    if accrued_rewards == Uint128::zero() {
        return Err(StdError::generic_err("No rewards to claim"));
    } else {
        messages.push(build_send_cw20_token_msg(
            sender_addr.clone(),
            config.mars_token,
            accrued_rewards,
        )?);
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        ("action", "Staking::ExecuteMsg::Claim"),
        ("user", sender_addr.as_str()),
        ("claimed_rewards", accrued_rewards.to_string().as_str()),
    ]))
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        mars_token: config.mars_token.to_string(),
        staking_token: config.staking_token.to_string(),
        init_timestamp: config.init_timestamp,
        till_timestamp: config.till_timestamp,
        cycle_duration: config.cycle_duration,
        reward_increase: config.reward_increase,
    })
}

/// @dev Returns the contract's simulated state at a certain timestamp
/// /// @param timestamp : Option parameter. Contract's Simulated state is retrieved if the timestamp is provided   
pub fn query_state(deps: Deps, env: Env, timestamp: Option<u64>) -> StdResult<StateResponse> {
    let mut state: State = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    match timestamp {
        Some(timestamp) => {
            
            // Timestamp cannot be from a past value
            if timestamp < env.block.time.seconds() {
                return Err(StdError::generic_err("Provided timestamp has passed"));
            }

            compute_reward(
                &config,
                &mut state,
                timestamp,
            );
        }
        None => {
            compute_reward(&config, &mut state, env.block.time.seconds());
        }
    }

    Ok(StateResponse {
        current_cycle: state.current_cycle,
        current_cycle_rewards: state.current_cycle_rewards,
        last_distributed: state.last_distributed,
        total_bond_amount: state.total_bond_amount,
        global_reward_index: state.global_reward_index,
    })
}

/// @dev Returns the User's simulated state at a certain timestamp
/// @param staker : User address whose state is to be retrieved
/// @param timestamp : Option parameter. User's Simulated state is retrieved if the timestamp is provided   
pub fn query_staker_info(
    deps: Deps,
    env: Env,
    staker: String,
    timestamp: Option<u64>,
) -> StdResult<StakerInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut staker_info = STAKER_INFO
        .may_load(deps.storage, &deps.api.addr_validate(&staker)?)?
        .unwrap_or_default();

    match timestamp {
        Some(timestamp) => {

            // Timestamp cannot be from a past value
            if timestamp < env.block.time.seconds() {
                return Err(StdError::generic_err("Provided timestamp has passed"));
            }
            
            compute_reward(
                &config,
                &mut state,
                std::cmp::max(timestamp, env.block.time.seconds()),
            );
        }
        None => {
            compute_reward(&config, &mut state, env.block.time.seconds());
        }
    }

    compute_staker_reward(&state, &mut staker_info)?;

    Ok(StakerInfoResponse {
        staker,
        reward_index: staker_info.reward_index,
        bond_amount: staker_info.bond_amount,
        pending_reward: staker_info.pending_reward,
    })
}

/// @dev Returns the current timestamp
pub fn query_timestamp(env: Env) -> StdResult<TimeResponse> {
    Ok(TimeResponse {
        timestamp: env.block.time.seconds(),
    })
}

//----------------------------------------------------------------------------------------
// Helper Functions
//----------------------------------------------------------------------------------------

/// @dev Increases total LP shares and user's staked LP shares by `amount`
fn increase_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint128) {
    state.total_bond_amount += amount;
    staker_info.bond_amount += amount;
}

/// @dev Decreases total LP shares and user's staked LP shares by `amount`
fn decrease_bond_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint128) {
    state.total_bond_amount = state
        .total_bond_amount
        .checked_sub(amount)
        .expect("total_bond_amount :: overflow on subtraction");
    staker_info.bond_amount = staker_info
        .bond_amount
        .checked_sub(amount)
        .expect("bond_amount :: overflow on subtraction");
}

/// @dev Computes total accrued rewards
fn compute_reward(config: &Config, state: &mut State, cur_timestamp: u64) {
    // If the reward distribution period is over
    if state.last_distributed == config.till_timestamp {
        return;
    }

    let mut last_distribution_cycle = state.current_cycle;
    state.current_cycle = calculate_cycles_elapsed(
        cur_timestamp,
        config.init_timestamp,
        config.cycle_duration,
        config.till_timestamp,
    );
    let mut rewards_to_distribute = Uint128::zero();
    let mut last_distribution_next_timestamp: u64; // 0 as u64;

    while state.current_cycle >= last_distribution_cycle {
        last_distribution_next_timestamp = std::cmp::min(
            config.till_timestamp,
            calculate_init_timestamp_for_cycle(
                config.init_timestamp,
                last_distribution_cycle + 1,
                config.cycle_duration,
            ),
        );
        rewards_to_distribute += rewards_distributed_for_cycle(
            Decimal::from_ratio(state.current_cycle_rewards, config.cycle_duration),
            std::cmp::max(state.last_distributed, config.init_timestamp),
            std::cmp::min(cur_timestamp, last_distribution_next_timestamp),
        );
        state.current_cycle_rewards = calculate_cycle_rewards(
            state.current_cycle_rewards,
            config.reward_increase,
            state.current_cycle == last_distribution_cycle,
        );
        state.last_distributed = std::cmp::min(cur_timestamp, last_distribution_next_timestamp);
        last_distribution_cycle += 1;
    }

    if state.last_distributed == config.till_timestamp {
        state.current_cycle_rewards = Uint128::zero();
    }

    if state.total_bond_amount == Uint128::zero() || config.init_timestamp > cur_timestamp {
        return;
    }

    state.global_reward_index = state.global_reward_index
        + Decimal::from_ratio(rewards_to_distribute, state.total_bond_amount)
}

fn calculate_cycles_elapsed(
    current_timestamp: u64,
    config_init_timestamp: u64,
    cycle_duration: u64,
    config_till_timestamp: u64,
) -> u64 {
    if config_init_timestamp >= current_timestamp {
        return 0u64;
    }
    let max_cycles = (config_till_timestamp - config_init_timestamp) / cycle_duration;

    let time_elapsed = current_timestamp - config_init_timestamp;
    std::cmp::min(max_cycles, time_elapsed / cycle_duration)
}

fn calculate_init_timestamp_for_cycle(
    config_init_timestamp: u64,
    current_cycle: u64,
    cycle_duration: u64,
) -> u64 {
    config_init_timestamp + (current_cycle * cycle_duration)
}

fn rewards_distributed_for_cycle(
    rewards_per_sec: Decimal,
    from_timestamp: u64,
    till_timestamp: u64,
) -> Uint128 {
    if till_timestamp <= from_timestamp {
        return Uint128::zero();
    }
    rewards_per_sec * Uint128::from(till_timestamp - from_timestamp)
}

fn calculate_cycle_rewards(
    current_cycle_rewards: Uint128,
    reward_increase_percent: Decimal,
    is_same_cycle: bool,
) -> Uint128 {
    if is_same_cycle {
        return current_cycle_rewards;
    }
    current_cycle_rewards + (current_cycle_rewards * reward_increase_percent)
}

/// @dev Computes user's accrued rewards
fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) -> StdResult<()> {
    let pending_reward = (staker_info.bond_amount * state.global_reward_index)
        - (staker_info.bond_amount * staker_info.reward_index);
    staker_info.reward_index = state.global_reward_index;
    staker_info.pending_reward += pending_reward;
    Ok(())
}

/// @dev Helper function to build `CosmosMsg` to send cw20 tokens to a recipient address
fn build_send_cw20_token_msg(
    recipient: Addr,
    token_contract_address: Addr,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_contract_address.into(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: recipient.into(),
            amount,
        })?,
        funds: vec![],
    }))
}

/// Used when unwrapping an optional address sent in a contract call by a user.
/// Validates addreess if present, otherwise uses a given default value.
fn option_string_to_addr(
    api: &dyn Api,
    option_string: Option<String>,
    default: Addr,
) -> StdResult<Addr> {
    match option_string {
        Some(input_addr) => api.addr_validate(&input_addr),
        None => Ok(default),
    }
}

fn zero_address() -> Addr {
    Addr::unchecked("")
}
