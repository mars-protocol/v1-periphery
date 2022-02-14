use std::ops::Div;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg,
    WasmQuery,
};

use mars_periphery::airdrop::ExecuteMsg::EnableClaims as AirdropEnableClaims;
use mars_periphery::auction::{
    CallbackMsg, ConfigResponse, Cw20HookMsg, MigrateMsg, ExecuteMsg, InstantiateMsg, QueryMsg, StateResponse,
    UpdateConfigMsg, UserInfoResponse,
};

use mars_periphery::helpers::{
    build_approve_cw20_msg, build_send_cw20_token_msg, build_transfer_cw20_token_msg,
    cw20_get_balance, option_string_to_addr,
};
use mars_periphery::lockdrop::ExecuteMsg::EnableClaims as LockdropEnableClaims;
use cw2::set_contract_version;

use astroport::asset::{Asset, AssetInfo};
use astroport::generator::{PendingTokenResponse, QueryMsg as GenQueryMsg};

use crate::state::{Config, State, UserInfo, CONFIG, STATE, USERS};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

const UUSD_DENOM: &str = "uusd";

// version info for migration info
const CONTRACT_NAME: &str = "mars_auction";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");


//----------------------------------------------------------------------------------------
// Entry points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // CHECK :: init_timestamp needs to be valid
    if msg.init_timestamp < env.block.time.seconds() {
        return Err(StdError::generic_err(format!(
            "Invalid timestamp. Current timestamp : {}",
            env.block.time.seconds()
        )));
    }


    if msg.mars_deposit_window > msg.ust_deposit_window {
        return Err(StdError::generic_err(
            "UST deposit window cannot be less than MARS deposit window",
        ));
    }


    // CHECK :: mars_vesting_duration needs to be valid
    if msg.mars_vesting_duration == 0u64 {
        return Err(StdError::generic_err(
            "mars_vesting_duration cannot be 0",
        ));
    }


    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        mars_token_address: deps.api.addr_validate(&msg.mars_token_address)?,
        astro_token_address: deps.api.addr_validate(&msg.astro_token_address)?,
        airdrop_contract_address: deps.api.addr_validate(&msg.airdrop_contract_address)?,
        lockdrop_contract_address: deps.api.addr_validate(&msg.lockdrop_contract_address)?,
        lp_token_address: None,
        astroport_lp_pool: None,
        mars_lp_staking_contract: None,
        generator_contract: deps.api.addr_validate(&msg.generator_contract)?,
        mars_rewards: Uint128::zero(),
        mars_vesting_duration: msg.mars_vesting_duration,
        lp_tokens_vesting_duration: msg.lp_tokens_vesting_duration,
        init_timestamp: msg.init_timestamp,
        mars_deposit_window: msg.mars_deposit_window,
        ust_deposit_window: msg.ust_deposit_window,
        withdrawal_window: msg.withdrawal_window,
    };

    let state = STATE.load(deps.storage).unwrap_or_default();

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig { new_config } => handle_update_config(deps, info, new_config),

        ExecuteMsg::DepositUst {} => handle_deposit_ust(deps, env, info),
        ExecuteMsg::WithdrawUst { amount } => handle_withdraw_ust(deps, env, info, amount),

        ExecuteMsg::AddLiquidityToAstroportPool { slippage } => {
            handle_init_pool(deps, env, info, slippage)
        }
        ExecuteMsg::StakeLpTokens {
            single_incentive_staking,
            dual_incentives_staking,
        } => handle_stake_lp_tokens(
            deps,
            env,
            info,
            single_incentive_staking,
            dual_incentives_staking,
        ),

        ExecuteMsg::ClaimRewards {
            withdraw_unlocked_shares,
        } => handle_claim_rewards_and_unlock(deps, env, info, withdraw_unlocked_shares),

        ExecuteMsg::Callback(msg) => _handle_callback(deps, env, info, msg),
    }
}

/// @dev Receive CW20 hook to accept cw20 token deposits via `Send`. Used to accept MARS  deposits via Airdrop / Lockdrop contracts
pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.mars_token_address {
        return Err(StdError::generic_err("Only mars tokens are received!"));
    }

    // CHECK ::: Amount needs to be valid
    if cw20_msg.amount.is_zero() {
        return Err(StdError::generic_err("Amount must be greater than 0"));
    }

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::DepositMarsTokens { user_address } => {
            // CHECK :: MARS deposits can happen only via airdrop / lockdrop contracts
            if config.airdrop_contract_address != cw20_msg.sender
                && config.lockdrop_contract_address != cw20_msg.sender
            {
                return Err(StdError::generic_err("Unauthorized"));
            }

            handle_deposit_mars_tokens(deps, env, info, user_address, cw20_msg.amount)
        }
        Cw20HookMsg::IncreaseMarsIncentives {} => {
            handle_increasing_mars_incentives(deps, cw20_msg.amount)
        }
    }
}

fn _handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> StdResult<Response> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(StdError::generic_err(
            "callbacks cannot be invoked externally",
        ));
    }
    match msg {
        CallbackMsg::UpdateStateOnLiquidityAdditionToPool { prev_lp_balance } => {
            update_state_on_liquidity_addition_to_pool(deps, env, prev_lp_balance)
        }
        CallbackMsg::UpdateStateOnRewardClaim {
            user_address,
            prev_mars_balance,
            prev_astro_balance,
            withdraw_lp_shares,
        } => update_state_on_reward_claim(
            deps,
            env,
            user_address,
            prev_mars_balance,
            prev_astro_balance,
            withdraw_lp_shares,
        ),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, env, address)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

//----------------------------------------------------------------------------------------
// Handle functions
//----------------------------------------------------------------------------------------

/// @dev Facilitates increasing MARS incentives which are to be distributed for partcipating in the auction
pub fn handle_increasing_mars_incentives(
    deps: DepsMut,
    amount: Uint128,
) -> Result<Response, StdError> {
    let state = STATE.load(deps.storage)?;
    let mut config = CONFIG.load(deps.storage)?;

    if state.lp_shares_minted > Uint128::zero() {
        return Err(StdError::generic_err(
            "MARS tokens are already being distributed",
        ));
    };

    config.mars_rewards += amount;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "mars_incentives_increased")
        .add_attribute("amount", amount))
}

/// @dev Admin function to update Configuration parameters
/// @param new_config : Same as UpdateConfigMsg struct
pub fn handle_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfigMsg,
) -> StdResult<Response> {
    let mut config = CONFIG.load(deps.storage)?;

    // CHECK :: ONLY OWNER CAN CALL THIS FUNCTION
    if info.sender != config.owner {
        return Err(StdError::generic_err("Only owner can update configuration"));
    }

    // UPDATE :: ADDRESSES IF PROVIDED
    config.owner = option_string_to_addr(deps.api, new_config.owner, config.clone().owner)?;

    // IF POOL ADDRESS PROVIDED :: Update and query LP token address from the pool
    if let Some(astroport_lp_pool) = new_config.astroport_lp_pool {
        config.astroport_lp_pool = Some(deps.api.addr_validate(&astroport_lp_pool)?);

        let pair_info: astroport::asset::PairInfo = deps
            .querier
            .query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: config.clone().astroport_lp_pool.unwrap().to_string(),
                msg: to_binary(&astroport::pair::QueryMsg::Pair {}).unwrap(),
            }))
            .unwrap();

        config.lp_token_address = Some(pair_info.liquidity_token);
    }

    if let Some(mars_lp_staking_contract) = new_config.mars_lp_staking_contract {
        config.mars_lp_staking_contract = Some(deps.api.addr_validate(&mars_lp_staking_contract)?);
    }

    config.generator_contract = option_string_to_addr(
        deps.api,
        new_config.generator_contract,
        config.clone().generator_contract,
    )?;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "Auction::ExecuteMsg::UpdateConfig"))
}

/// @dev Accepts MARS tokens to be used for the LP Bootstrapping via auction. Callable only by Airdrop / Lockdrop contracts
/// @param user_address : User address who is delegating the MARS tokens for LP Pool bootstrap via auction
/// @param amount : Number of MARS Tokens being deposited
pub fn handle_deposit_mars_tokens(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_address: Addr,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: MARS delegations window open
    let mars_delegations_allowed_till = config.init_timestamp + config.mars_deposit_window;
    if !(config.init_timestamp <= env.block.time.seconds()
        && env.block.time.seconds() <= mars_delegations_allowed_till)
    {
        return Err(StdError::generic_err("MARS delegation window closed"));
    }

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // UPDATE STATE
    state.total_mars_deposited += amount;
    user_info.mars_deposited += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::DepositMarsTokens"),
        attr("user", user_address.to_string()),
        attr("mars_deposited", amount),
    ]))
}

/// @dev Facilitates UST deposits by users to be used for LP Bootstrapping via auction
pub fn handle_deposit_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;

    // CHECK :: UST deposits window open
    let ust_deposits_allowed_till = config.init_timestamp + config.ust_deposit_window;
    if !(config.init_timestamp <= env.block.time.seconds()
        && env.block.time.seconds() <= ust_deposits_allowed_till)
    {
        return Err(StdError::generic_err("UST deposits window closed"));
    }

    let mut state = STATE.load(deps.storage)?;
    let mut user_info = USERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_default();

    // Check if multiple native coins sent by the user
    if info.funds.len() > 1 {
        return Err(StdError::generic_err("Trying to deposit several coins"));
    }

    // Only UST accepted and amount > 0
    let native_token = info.funds.first().unwrap();
    if native_token.denom != *UUSD_DENOM {
        return Err(StdError::generic_err(
            "Only UST among native tokens accepted",
        ));
    }

    if native_token.amount.is_zero() {
        return Err(StdError::generic_err(
            "Deposit amount must be greater than 0",
        ));
    }

    // UPDATE STATE
    state.total_ust_deposited += native_token.amount;
    user_info.ust_deposited += native_token.amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &info.sender, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        attr("action", "Auction::ExecuteMsg::deposit_ust"),
        attr("user_address", info.sender.to_string()),
        attr("ust_deposited", native_token.amount.to_string()),
    ]))
}

/// @dev Facilitates UST withdrawals by users from their deposit positions
/// @param amount : UST amount being withdrawn
pub fn handle_withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Has the user already withdrawn during the current window
    if user_info.ust_withdrawn_flag {
        return Err(StdError::generic_err(
            "Max 1 withdrawal allowed during current window",
        ));
    }

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent = allowed_withdrawal_percent(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.ust_deposited * max_withdrawal_percent;

    if amount > max_withdrawal_allowed {
        return Err(StdError::generic_err(format!(
            "Amount exceeds maximum allowed withdrawal limit of {} uusd",
            max_withdrawal_allowed
        )));
    }

    // After UST deposit window is closed, we allow to withdraw only once
    if env.block.time.seconds() > config.init_timestamp + config.ust_deposit_window {
        user_info.ust_withdrawn_flag = true;
    }

    // UPDATE STATE
    state.total_ust_deposited = state.total_ust_deposited.checked_sub(amount)?;
    user_info.ust_deposited = user_info.ust_deposited.checked_sub(amount)?;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USERS.save(deps.storage, &user_address, &user_info)?;

    // COSMOSMSG :: Transfer UST to the user
    let transfer_ust = Asset {
        amount,
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    }
    .into_msg(&deps.querier, user_address.clone())?;

    Ok(Response::new()
        .add_message(transfer_ust)
        .add_attributes(vec![
            attr("action", "Auction::ExecuteMsg::withdraw_ust"),
            attr("user", user_address.to_string()),
            attr("ust_withdrawn_flag", amount),
        ]))
}

/// @dev Admin function to bootstrap the MARS-UST Liquidity pool by depositing all MARS, UST tokens deposited to the Astroport pool
/// @param slippage Optional, to handle slippage that may be there when adding liquidity to the pool
pub fn handle_init_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    slippage: Option<Decimal>,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // CHECK :: Only admin can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Liquidity already provided to pool"));
    if !state.lp_shares_minted.is_zero() {
        return Err(StdError::generic_err("Liquidity already provided to pool"));
    }

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err(
            "Deposit/withdrawal windows are still open",
        ));
    }

    // CHECK :: LP Pool addresses should be set
    if config.astroport_lp_pool.is_none() {
        return Err(StdError::generic_err(
            "Pool address to which liquidity is to be migrated not set",
        ));
    }

    // Init response
    let mut response =
        Response::new().add_attribute("action", "Auction::ExecuteMsg::AddLiquidityToAstroportPool");

    // QUERY CURRENT LP TOKEN BALANCE (FOR SAFETY - IN ANY CASE)
    let cur_lp_balance = cw20_get_balance(
        &deps.querier,
        config.lp_token_address.clone().unwrap(),
        env.contract.address.clone(),
    )?;

    // COSMOS MSGS
    // :: 1.  APPROVE MARS WITH LP POOL ADDRESS AS BENEFICIARY
    // :: 2.  ADD LIQUIDITY
    // :: 3. CallbackMsg :: Update state on liquidity addition to LP Pool
    // :: 4. Activate Claims on Lockdrop Contract (In Callback)
    // :: 5. Update Claims on Airdrop Contract (In Callback)
    let approve_mars_msg = build_approve_cw20_msg(
        config.mars_token_address.to_string(),
        config.astroport_lp_pool.clone().unwrap().to_string(),
        state.total_mars_deposited,
    )?;
    let add_liquidity_msg =
        build_provide_liquidity_to_lp_pool_msg(deps.as_ref(), config, &state, slippage)?;

    let update_state_msg = CallbackMsg::UpdateStateOnLiquidityAdditionToPool {
        prev_lp_balance: cur_lp_balance,
    }
    .to_cosmos_msg(&env.contract.address)?;

    response = response
        .add_messages(vec![approve_mars_msg, add_liquidity_msg, update_state_msg])
        .add_attribute("mars_deposited", state.total_mars_deposited)
        .add_attribute("ust_deposited", state.total_ust_deposited);

    Ok(response)
}

/// @dev Admin function to stake Astroport LP tokens with the generator contract
/// @params single_incentive_staking : Boolean value indicating if LP Tokens are to be staked with MARS LP Contract or not
/// @params dual_incentives_staking : Boolean value indicating if LP Tokens are to be staked with Astroport Generator Contract or not
pub fn handle_stake_lp_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    single_incentive_staking: bool,
    dual_incentives_staking: bool,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut are_being_unstaked = false;

    // CHECK :: Only admin can call this function
    if info.sender != config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    // CHECK :: Check if valid boolean values are provided or not
    if (single_incentive_staking && dual_incentives_staking)
        || (!single_incentive_staking && !dual_incentives_staking)
    {
        return Err(StdError::generic_err("Invalid values provided"));
    }

    // Init response
    let mut response =
        Response::new().add_attribute("action", "Auction::ExecuteMsg::StakeLPTokens");

    // CHECK :: Check if already staked with MARS LP Staking contracts
    if single_incentive_staking && state.are_staked_for_single_incentives {
        return Err(StdError::generic_err(
            "LP Tokens already staked with MARS LP Staking contract",
        ));
    }

    // CHECK :: Check if already staked with MARS Generator
    if dual_incentives_staking && state.are_staked_for_dual_incentives {
        return Err(StdError::generic_err(
            "LP Tokens already staked with Astroport Generator",
        ));
    }

    // IF TO BE STAKED WITH MARS LP STAKING CONTRACT
    if single_incentive_staking {
        let lp_shares_balance = state.lp_shares_minted - state.lp_shares_withdrawn;

        // Unstake from Generator contract (if staked)
        if state.are_staked_for_dual_incentives {
            response = response
                .add_message(build_unstake_from_generator_msg(
                    &config,
                    lp_shares_balance,
                )?)
                .add_attribute(
                    "shares_withdrawn_from_generator",
                    lp_shares_balance.to_string(),
                );
            are_being_unstaked = true;
        }

        // Check if LP Staking contract is set
        if config.mars_lp_staking_contract.is_none() {
            return Err(StdError::generic_err("LP Staking not set"));
        }

        // :: Add stake LP Tokens to the MARS LP Staking contract msg
        let stake_msg =
            build_stake_with_mars_staking_contract_msg(config.clone(), lp_shares_balance)?;

        response = response
            .add_message(stake_msg)
            .add_attribute("shares_staked_with_lp_contract", "true")
            .add_attribute("shares_staked_amount", lp_shares_balance.to_string());

        // Update boolean values which indicate where the LP tokens are staked
        state.are_staked_for_single_incentives = true;
        state.are_staked_for_dual_incentives = false;
    }

    // IF TO BE STAKED WITH GENERATOR
    if dual_incentives_staking {
        let lp_shares_balance = state.lp_shares_minted - state.lp_shares_withdrawn;

        // Unstake from LP Staking contract (if staked)
        if state.are_staked_for_single_incentives {
            response = response
                .add_message(build_unstake_from_mars_staking_contract_msg(
                    config
                        .mars_lp_staking_contract
                        .clone()
                        .expect("LP Staking contract not set")
                        .to_string(),
                    lp_shares_balance,
                    true,
                )?)
                .add_attribute(
                    "shares_unstaked_from_lp_staking",
                    lp_shares_balance.to_string(),
                );
            are_being_unstaked = true;
        }

        // COSMOS MSGs
        // :: Add increase allowance msg so generator contract can transfer tokens to itself
        // :: Add stake LP Tokens to the Astroport generator contract msg
        let approve_msg = build_approve_cw20_msg(
            config.lp_token_address.clone().unwrap().to_string(),
            config.generator_contract.to_string(),
            lp_shares_balance,
        )?;
        let stake_msg = build_stake_with_generator_msg(config.clone(), lp_shares_balance)?;
        response = response
            .add_messages(vec![approve_msg, stake_msg])
            .add_attribute("shares_staked_with_generator", "true")
            .add_attribute("shares_staked_amount", lp_shares_balance.to_string());

        // Update boolean values which indicate where the LP tokens are staked
        state.are_staked_for_dual_incentives = true;
        state.are_staked_for_single_incentives = false;
    }

    if are_being_unstaked {
        // --> Add CallbackMsg::UpdateStateOnRewardClaim msg to the cosmos msg array
        let mars_balance = cw20_get_balance(
            &deps.querier,
            config.mars_token_address,
            env.contract.address.clone(),
        )?;
        let astro_balance = cw20_get_balance(
            &deps.querier,
            config.astro_token_address,
            env.contract.address.clone(),
        )?;
        let update_state_msg = CallbackMsg::UpdateStateOnRewardClaim {
            user_address: None,
            prev_mars_balance: mars_balance,
            prev_astro_balance: astro_balance,
            withdraw_lp_shares: Uint128::zero(),
        }
        .to_cosmos_msg(&env.contract.address)?;
        response = response.add_message(update_state_msg);
    }

    STATE.save(deps.storage, &state)?;

    Ok(response)
}

/// @dev Facilitates MARS/ASTRO Reward claim for users
/// @params withdraw_unlocked_shares : Boolean value indicating if the vested Shares are to be withdrawn or not
pub fn handle_claim_rewards_and_unlock(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_unlocked_shares: bool,
) -> Result<Response, StdError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    let user_address = info.sender;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    // CHECK :: Deposit / withdrawal windows need to be over
    if !are_windows_closed(env.block.time.seconds(), &config) {
        return Err(StdError::generic_err("Deposit/withdrawal windows are open"));
    }

    // CHECK :: Does user have valid MARS / UST deposit balances
    if user_info.mars_deposited == Uint128::zero() && user_info.ust_deposited == Uint128::zero() {
        return Err(StdError::generic_err("Invalid request"));
    }

    // Init response
    let mut response = Response::new()
        .add_attribute("action", "Auction::ExecuteMsg::ClaimRewards")
        .add_attribute("user_address", user_address.to_string())
        .add_attribute("withdraw_lp_shares", withdraw_unlocked_shares.to_string());

    // LP SHARES :: Calculate if not already calculated
    if user_info.lp_shares == Uint128::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
        response = response.add_attribute("user_lp_share", user_info.lp_shares.to_string());
    }

    // MARS INCENTIVES :: Calculates MARS rewards for auction participation for a user if not already done
    if user_info.total_auction_incentives == Uint128::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.mars_rewards);
        response = response.add_attribute(
            "user_total_auction_mars_incentive",
            user_info.total_auction_incentives.to_string(),
        );
    }

    let mut lp_shares_to_withdraw = Uint128::zero();
    if withdraw_unlocked_shares {
        lp_shares_to_withdraw =
            calculate_withdrawable_lp_shares(env.block.time.seconds(), &config, &state, &user_info);
    }

    // --> IF LP TOKENS are staked with MARS LP Staking contract
    if state.are_staked_for_single_incentives {
        let unclaimed_rewards_response = query_unclaimed_staking_rewards_at_mars_lp_staking(
            &deps.querier,
            config
                .mars_lp_staking_contract
                .clone()
                .expect("LP Staking contract not set")
                .to_string(),
            env.contract.address.clone(),
        );

        if unclaimed_rewards_response.pending_reward > Uint128::zero() || withdraw_unlocked_shares {
            let claim_reward_msg: CosmosMsg;
            // If LP tokens are to be withdrawn. We unstake the equivalent amount. Rewards are automatically claimed with the call
            if withdraw_unlocked_shares {
                claim_reward_msg = build_unstake_from_mars_staking_contract_msg(
                    config
                        .mars_lp_staking_contract
                        .clone()
                        .expect("LP Staking contract not set")
                        .to_string(),
                    lp_shares_to_withdraw,
                    true,
                )?;
            }
            // If only rewards are to be claimed
            else {
                claim_reward_msg = build_claim_rewards_from_mars_staking_contract_msg(
                    config
                        .mars_lp_staking_contract
                        .clone()
                        .expect("LP Staking contract not set")
                        .to_string(),
                )?;
            }
            response = response
                .add_message(claim_reward_msg)
                .add_attribute("claim_rewards", "mars_staking_contract");
        }
    }

    // --> IF LP TOKENS are staked with Generator contract
    if state.are_staked_for_dual_incentives {
        let unclaimed_rewards_response: astroport::generator::PendingTokenResponse =
            query_unclaimed_staking_rewards_at_generator(
                &deps.querier,
                &config,
                env.contract.address.clone(),
            );

        if unclaimed_rewards_response.pending > Uint128::zero()
            || (unclaimed_rewards_response.pending_on_proxy.is_some()
                && unclaimed_rewards_response.pending_on_proxy.unwrap() > Uint128::zero())
            || withdraw_unlocked_shares
        {
            let claim_reward_msg =
                build_unstake_from_generator_msg(&config, lp_shares_to_withdraw)?;
            response = response
                .add_message(claim_reward_msg)
                .add_attribute("claim_rewards", "generator");
        }
    }

    // --> Add CallbackMsg::UpdateStateOnRewardClaim msg to the cosmos msg array
    let mars_balance = cw20_get_balance(
        &deps.querier,
        config.mars_token_address,
        env.contract.address.clone(),
    )?;
    let astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address,
        env.contract.address.clone(),
    )?;
    let update_state_msg = CallbackMsg::UpdateStateOnRewardClaim {
        user_address: Some(user_address.clone()),
        prev_mars_balance: mars_balance,
        prev_astro_balance: astro_balance,
        withdraw_lp_shares: lp_shares_to_withdraw,
    }
    .to_cosmos_msg(&env.contract.address)?;
    response = response.add_message(update_state_msg);

    USERS.save(deps.storage, &user_address, &user_info)?;

    Ok(response)
}

//----------------------------------------------------------------------------------------
// Handle::Callback functions
//----------------------------------------------------------------------------------------

/// @dev Callback function. Updates state after initialization of MARS-UST Pool
/// @params prev_lp_balance : Astro LP Token balance before pool initialization
pub fn update_state_on_liquidity_addition_to_pool(
    deps: DepsMut,
    env: Env,
    prev_lp_balance: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // QUERY CURRENT LP TOKEN BALANCE :: NEWLY MINTED LP TOKENS
    let cur_lp_balance = cw20_get_balance(
        &deps.querier,
        config.lp_token_address.expect("LP Token not set"),
        env.contract.address,
    )?;

    // STATE :: UPDATE --> SAVE
    state.lp_shares_minted = cur_lp_balance - prev_lp_balance;
    state.pool_init_timestamp = env.block.time.seconds();
    STATE.save(deps.storage, &state)?;

    let mut cosmos_msgs = vec![];
    let activate_claims_lockdrop =
        build_activate_claims_lockdrop_msg(config.lockdrop_contract_address)?;
    let activate_claims_airdrop =
        build_activate_claims_airdrop_msg(config.airdrop_contract_address)?;
    cosmos_msgs.push(activate_claims_lockdrop);
    cosmos_msgs.push(activate_claims_airdrop);

    Ok(Response::new()
        .add_messages(cosmos_msgs)
        .add_attributes(vec![
            (
                "action",
                "Auction::CallbackMsg::UpdateStateOnLiquidityAddition",
            ),
            (
                "lp_shares_minted",
                state.lp_shares_minted.to_string().as_str(),
            ),
        ]))
}

// @dev CallbackMsg :: Facilitates state update and MARS / ASTRO rewards transfer to users post MARS incentives claim from the generator contract
pub fn update_state_on_reward_claim(
    deps: DepsMut,
    env: Env,
    user_address: Option<Addr>,
    prev_mars_balance: Uint128,
    prev_astro_balance: Uint128,
    withdraw_lp_shares: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // Claimed Rewards :: QUERY MARS & ASTRO TOKEN BALANCE
    let cur_mars_balance = cw20_get_balance(
        &deps.querier,
        config.mars_token_address.clone(),
        env.contract.address.clone(),
    )?;
    let cur_astro_balance = cw20_get_balance(
        &deps.querier,
        config.astro_token_address.clone(),
        env.contract.address.clone(),
    )?;
    let mars_claimed = cur_mars_balance.checked_sub(prev_mars_balance)?;
    let astro_claimed = cur_astro_balance.checked_sub(prev_astro_balance)?;

    // Update Global Reward Indexes
    update_mars_rewards_index(&mut state, mars_claimed);
    update_astro_rewards_index(&mut state, astro_claimed);

    // Init response
    let mut response = Response::new()
        .add_attribute("total_claimed_mars", mars_claimed.to_string())
        .add_attribute("total_claimed_astro", astro_claimed.to_string());

    // IF VALID USER ADDRESSES (All cases except staking() function call)
    if let Some(user_address) = user_address {
        let mut user_info = USERS
            .may_load(deps.storage, &user_address)?
            .unwrap_or_default();

        // MARS Incentives :: Calculate the unvested amount which can be claimed by the user
        let mut user_mars_rewards = calculate_withdrawable_auction_reward_for_user(
            env.block.time.seconds(),
            &config,
            &state,
            &user_info,
        );
        user_info.withdrawn_auction_incentives += user_mars_rewards;
        response = response.add_attribute(
            "withdrawn_auction_incentives",
            user_mars_rewards.to_string(),
        );

        // MARS (Staking) rewards :: Calculate the amount (from LP staking incentives) which can be claimed by the user
        let staking_reward_mars = compute_user_accrued_mars_reward(&state, &mut user_info);
        user_info.withdrawn_mars_incentives += staking_reward_mars;
        user_mars_rewards += staking_reward_mars;
        response = response.add_attribute("user_mars_incentives", staking_reward_mars.to_string());

        // ASTRO (Staking) rewards :: Calculate the amount (from LP staking incentives) which can be claimed by the user
        let staking_reward_astro = compute_user_accrued_astro_reward(&state, &mut user_info);
        user_info.withdrawn_astro_incentives += staking_reward_astro;
        response =
            response.add_attribute("user_astro_incentives", staking_reward_astro.to_string());

        // COSMOS MSG :: Transfer $MARS to the user
        if user_mars_rewards > Uint128::zero() {
            let transfer_mars_rewards = build_transfer_cw20_token_msg(
                user_address.clone(),
                config.mars_token_address.to_string(),
                user_mars_rewards,
            )?;
            response = response.add_message(transfer_mars_rewards);
        }

        // COSMOS MSG :: Transfer $ASTRO to the user
        if staking_reward_astro > Uint128::zero() {
            let transfer_astro_rewards = build_transfer_cw20_token_msg(
                user_address.clone(),
                config.astro_token_address.to_string(),
                staking_reward_astro,
            )?;
            response = response.add_message(transfer_astro_rewards);
        }

        // COSMOS MSG :: WITHDRAW LP Shares
        if withdraw_lp_shares > Uint128::zero() {
            let transfer_lp_shares = build_transfer_cw20_token_msg(
                user_address.clone(),
                config
                    .lp_token_address
                    .expect("LP Token not set")
                    .to_string(),
                withdraw_lp_shares,
            )?;
            response = response.add_message(transfer_lp_shares);

            user_info.withdrawn_lp_shares += withdraw_lp_shares;
            state.lp_shares_withdrawn += withdraw_lp_shares;
        }

        USERS.save(deps.storage, &user_address, &user_info)?;
    }

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;

    Ok(response)
}

//----------------------------------------------------------------------------------------
// Query functions
//----------------------------------------------------------------------------------------

/// @dev Returns the airdrop configuration
fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        mars_token_address: config.mars_token_address.to_string(),
        astro_token_address: config.astro_token_address.to_string(),
        airdrop_contract_address: config.airdrop_contract_address.to_string(),
        lockdrop_contract_address: config.lockdrop_contract_address.to_string(),
        astroport_lp_pool: config.astroport_lp_pool,
        lp_token_address: config.lp_token_address,
        mars_lp_staking_contract: config.mars_lp_staking_contract,
        generator_contract: config.generator_contract.to_string(),
        mars_rewards: config.mars_rewards,
        mars_vesting_duration: config.mars_vesting_duration,
        lp_tokens_vesting_duration: config.lp_tokens_vesting_duration,
        init_timestamp: config.init_timestamp,
        mars_deposit_window: config.mars_deposit_window,
        ust_deposit_window: config.ust_deposit_window,
        withdrawal_window: config.withdrawal_window,
    })
}

/// @dev Returns the airdrop contract state
fn query_state(deps: Deps) -> StdResult<StateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_mars_deposited: state.total_mars_deposited,
        total_ust_deposited: state.total_ust_deposited,
        lp_shares_minted: state.lp_shares_minted,
        lp_shares_withdrawn: state.lp_shares_withdrawn,
        are_staked_for_single_incentives: state.are_staked_for_single_incentives,
        are_staked_for_dual_incentives: state.are_staked_for_dual_incentives,
        pool_init_timestamp: state.pool_init_timestamp,
        global_mars_reward_index: state.global_mars_reward_index,
        global_astro_reward_index: state.global_astro_reward_index,
    })
}

/// @dev Returns details around user's MARS Airdrop claim
fn query_user_info(deps: Deps, env: Env, user_address: String) -> StdResult<UserInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user_address)?;
    let mut user_info = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    if user_info.lp_shares == Uint128::zero() {
        user_info.lp_shares = calculate_user_lp_share(&state, &user_info);
    }

    if user_info.total_auction_incentives == Uint128::zero() {
        user_info.total_auction_incentives =
            calculate_auction_reward_for_user(&state, &user_info, config.mars_rewards);
    }
    let withdrawable_lp_shares =
        calculate_withdrawable_lp_shares(env.block.time.seconds(), &config, &state, &user_info);
    let claimable_auction_reward = calculate_withdrawable_auction_reward_for_user(
        env.block.time.seconds(),
        &config,
        &state,
        &user_info,
    );

    let mut withdrawable_mars_incentives = Uint128::zero();
    let mut withdrawable_astro_incentives = Uint128::zero();

    // --> IF LP TOKENS are staked with MARS LP STaking contract
    if state.are_staked_for_single_incentives {
        let unclaimed_rewards_response = query_unclaimed_staking_rewards_at_mars_lp_staking(
            &deps.querier,
            config
                .mars_lp_staking_contract
                .clone()
                .expect("LP Staking contract not set")
                .to_string(),
            env.contract.address.clone(),
        );
        update_mars_rewards_index(&mut state, unclaimed_rewards_response.pending_reward);
        withdrawable_mars_incentives = compute_user_accrued_mars_reward(&state, &mut user_info);
    }

    // --> IF LP TOKENS are staked with Generator contract
    if state.are_staked_for_dual_incentives {
        let unclaimed_rewards_response = query_unclaimed_staking_rewards_at_generator(
            &deps.querier,
            &config,
            env.contract.address,
        );
        update_mars_rewards_index(
            &mut state,
            unclaimed_rewards_response.pending_on_proxy.unwrap(),
        );
        withdrawable_mars_incentives = compute_user_accrued_mars_reward(&state, &mut user_info);

        update_astro_rewards_index(&mut state, unclaimed_rewards_response.pending);
        withdrawable_astro_incentives = compute_user_accrued_astro_reward(&state, &mut user_info);
    }

    Ok(UserInfoResponse {
        mars_deposited: user_info.mars_deposited,
        ust_deposited: user_info.ust_deposited,
        ust_withdrawn_flag: user_info.ust_withdrawn_flag,
        lp_shares: user_info.lp_shares,
        withdrawn_lp_shares: user_info.withdrawn_lp_shares,
        withdrawable_lp_shares,
        total_auction_incentives: user_info.total_auction_incentives,
        withdrawn_auction_incentives: user_info.withdrawn_auction_incentives,
        withdrawable_auction_incentives: claimable_auction_reward,
        mars_reward_index: user_info.mars_reward_index,
        withdrawable_mars_incentives,
        withdrawn_mars_incentives: user_info.withdrawn_mars_incentives,
        astro_reward_index: user_info.astro_reward_index,
        withdrawable_astro_incentives,
        withdrawn_astro_incentives: user_info.withdrawn_astro_incentives,
    })
}

//----------------------------------------------------------------------------------------
// HELPERS :: LP & REWARD CALCULATIONS
//----------------------------------------------------------------------------------------

/// @dev Calculates user's MARS-UST LP Shares
/// Formula -
/// user's MARS share %  = user's MARS deposits / Total MARS deposited
/// user's UST share %  = user's UST deposits / Total UST deposited
/// user's LP balance  = ( user's MARS share % + user's UST share % ) / 2 * Total LPs Minted
/// @param state : Contract State
/// @param user_info : User Info State
fn calculate_user_lp_share(state: &State, user_info: &UserInfo) -> Uint128 {
    if state.total_mars_deposited == Uint128::zero() || state.total_ust_deposited == Uint128::zero()
    {
        return user_info.lp_shares;
    }
    let user_mars_shares_percent =
        Decimal::from_ratio(user_info.mars_deposited, state.total_mars_deposited);
    let user_ust_shares_percent =
        Decimal::from_ratio(user_info.ust_deposited, state.total_ust_deposited);
    let user_total_share_percent = user_mars_shares_percent + user_ust_shares_percent;

    user_total_share_percent.div(Uint128::from(2u64)) * state.lp_shares_minted
}

/// @dev Calculates MARS tokens receivable by a user for delegating MARS & depositing UST in the bootstraping phase of the MARS-UST Pool
/// Formula -
/// user's MARS share %  = user's MARS deposits / Total MARS deposited
/// user's UST share %  = user's UST deposits / Total UST deposited
/// user's Auction Reward  = ( user's MARS share % + user's UST share % ) / 2 * Total Auction Incentives
/// @param total_mars_rewards : Total MARS tokens to be distributed as auction participation reward
fn calculate_auction_reward_for_user(
    state: &State,
    user_info: &UserInfo,
    total_mars_rewards: Uint128,
) -> Uint128 {
    let mut user_mars_shares_percent = Decimal::zero();
    let mut user_ust_shares_percent = Decimal::zero();

    if user_info.mars_deposited > Uint128::zero() {
        user_mars_shares_percent =
            Decimal::from_ratio(user_info.mars_deposited, state.total_mars_deposited);
    }
    if user_info.ust_deposited > Uint128::zero() {
        user_ust_shares_percent =
            Decimal::from_ratio(user_info.ust_deposited, state.total_ust_deposited);
    }
    let user_total_share_percent = user_mars_shares_percent + user_ust_shares_percent;
    user_total_share_percent.div(Uint128::from(2u64)) * total_mars_rewards
}

/// @dev Returns LP Balance that a user can withdraw based on the vesting schedule
/// Formula -
/// time elapsed = current timestamp - timestamp when liquidity was added to the MARS-UST LP Pool
/// Total LP shares that a user can withdraw =  User's LP shares *  time elapsed / vesting duration
/// LP shares that a user can currently withdraw =  Total LP shares that a user can withdraw  - LP shares withdrawn
/// @param current_timestamp : Current timestamp
/// @param user_info : User Info State
pub fn calculate_withdrawable_lp_shares(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint128 {
    if state.pool_init_timestamp == 0u64 {
        return Uint128::zero();
    }
    let time_elapsed = cur_timestamp - state.pool_init_timestamp;

    if time_elapsed >= config.lp_tokens_vesting_duration {
        return user_info.lp_shares - user_info.withdrawn_lp_shares;
    }

    let withdrawable_lp_balance =
        user_info.lp_shares * Decimal::from_ratio(time_elapsed, config.lp_tokens_vesting_duration);
    withdrawable_lp_balance - user_info.withdrawn_lp_shares
}

/// @dev Returns MARS auction incentives that a user can withdraw based on the vesting schedule
/// Formula -
/// time elapsed = current timestamp - timestamp when liquidity was added to the MARS-UST LP Pool
/// Total MARS that a user can withdraw =  User's MARS reward *  time elapsed / vesting duration
/// MARS rewards that a user can currently withdraw =  Total MARS rewards that a user can withdraw  - MARS rewards withdrawn
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
/// @param state : Contract State
/// @param user_info : User Info State
pub fn calculate_withdrawable_auction_reward_for_user(
    cur_timestamp: u64,
    config: &Config,
    state: &State,
    user_info: &UserInfo,
) -> Uint128 {
    if user_info.withdrawn_auction_incentives == user_info.total_auction_incentives
        || state.pool_init_timestamp == 0u64
    {
        return Uint128::zero();
    }

    let time_elapsed = cur_timestamp - state.pool_init_timestamp;
    if time_elapsed >= config.mars_vesting_duration {
        return user_info.total_auction_incentives - user_info.withdrawn_auction_incentives;
    }
    let withdrawable_auction_incentives = user_info.total_auction_incentives
        * Decimal::from_ratio(time_elapsed, config.mars_vesting_duration);
    withdrawable_auction_incentives - user_info.withdrawn_auction_incentives
}

/// @dev Accrue MARS rewards by updating the global mars reward index
/// Formula ::: global mars reward index += MARS accrued / (LP shares staked)
fn update_mars_rewards_index(state: &mut State, mars_accured: Uint128) {
    let staked_lp_shares = state.lp_shares_minted - state.lp_shares_withdrawn;
    if staked_lp_shares == Uint128::zero() {
        return;
    }
    state.global_mars_reward_index =
        state.global_mars_reward_index + Decimal::from_ratio(mars_accured, staked_lp_shares);
}

/// @dev Accrue ASTRO rewards by updating the global astro reward index
/// Formula ::: global astro reward index += ASTRO accrued / (LP shares staked)
fn update_astro_rewards_index(state: &mut State, astro_accured: Uint128) {
    let staked_lp_shares = state.lp_shares_minted - state.lp_shares_withdrawn;
    if staked_lp_shares == Uint128::zero() {
        return;
    }
    state.global_astro_reward_index =
        state.global_astro_reward_index + Decimal::from_ratio(astro_accured, staked_lp_shares);
}

/// @dev Accrue MARS reward for the user by updating the user reward index and adding rewards to the pending rewards
/// Formula :: Pending user mars rewards = (user's staked LP shares) * ( global mars reward index - user mars reward index )
fn compute_user_accrued_mars_reward(state: &State, user_info: &mut UserInfo) -> Uint128 {
    let staked_lp_shares = user_info.lp_shares - user_info.withdrawn_lp_shares;

    let pending_user_rewards = (staked_lp_shares * state.global_mars_reward_index)
        - (staked_lp_shares * user_info.mars_reward_index);
    user_info.mars_reward_index = state.global_mars_reward_index;
    pending_user_rewards
}

/// @dev Accrue ASTRO reward for the user by updating the user reward index
/// Formula :: Pending user astro rewards = (user's staked LP shares) * ( global astro reward index - user astro reward index )
fn compute_user_accrued_astro_reward(state: &State, user_info: &mut UserInfo) -> Uint128 {
    let staked_lp_shares = user_info.lp_shares - user_info.withdrawn_lp_shares;
    let pending_user_rewards = (staked_lp_shares * state.global_astro_reward_index)
        - (staked_lp_shares * user_info.astro_reward_index);
    user_info.astro_reward_index = state.global_astro_reward_index;
    pending_user_rewards
}

//----------------------------------------------------------------------------------------
// HELPERS :: DEPOSIT / WITHDRAW CALCULATIONS
//----------------------------------------------------------------------------------------

/// @dev Helper function. Returns true if the deposit & withdrawal windows are closed, else returns false
/// @param current_timestamp : Current timestamp
/// @param config : Configuration
fn are_windows_closed(current_timestamp: u64, config: &Config) -> bool {
    let opened_till = config.init_timestamp + config.ust_deposit_window + config.withdrawal_window;
    (current_timestamp > opened_till) || (current_timestamp < config.init_timestamp)
}

///  @dev Helper function to calculate maximum % of their total UST deposited that can be withdrawn.  Returns % UST that can be withdrawn
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
fn allowed_withdrawal_percent(current_timestamp: u64, config: &Config) -> Decimal {
    let ust_withdrawal_cutoff_init_point = config.init_timestamp + config.ust_deposit_window;

    // Deposit window :: 100% withdrawals allowed
    if current_timestamp <= ust_withdrawal_cutoff_init_point {
        return Decimal::from_ratio(100u32, 100u32);
    }

    let ust_withdrawal_cutoff_second_point =
        ust_withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    // Deposit window closed, 1st half of withdrawal window :: 50% withdrawals allowed
    if current_timestamp <= ust_withdrawal_cutoff_second_point {
        return Decimal::from_ratio(50u32, 100u32);
    }
    let ust_withdrawal_cutoff_final =
        ust_withdrawal_cutoff_second_point + (config.withdrawal_window / 2u64);
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < ust_withdrawal_cutoff_final {
        let time_left = ust_withdrawal_cutoff_final - current_timestamp;
        Decimal::from_ratio(
            50u64 * time_left,
            100u64 * (ust_withdrawal_cutoff_final - ust_withdrawal_cutoff_second_point),
        )
    }
    // Withdrawals not allowed
    else {
        Decimal::from_ratio(0u32, 100u32)
    }
}

//----------------------------------------------------------------------------------------
// HELPERS :: QUERIES
//----------------------------------------------------------------------------------------

/// @dev Queries pending rewards to be claimed from the generator contract for the 'contract_addr'
/// @param config : Configuration
/// @param contract_addr : Address for which pending rewards are to be queried
fn query_unclaimed_staking_rewards_at_generator(
    querier: &QuerierWrapper,
    config: &Config,
    contract_addr: Addr,
) -> astroport::generator::PendingTokenResponse {
    let pending_rewards: PendingTokenResponse = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.generator_contract.to_string(),
            msg: to_binary(&GenQueryMsg::PendingToken {
                lp_token: config.lp_token_address.clone().expect("LP Token not set"),
                user: contract_addr,
            })
            .unwrap(),
        }))
        .unwrap();
    pending_rewards
}

/// @dev Queries pending rewards to be claimed from the MARS LP Staking contract
/// @param config : Configuration
/// @param contract_addr : Address for which pending rewards are to be queried
fn query_unclaimed_staking_rewards_at_mars_lp_staking(
    querier: &QuerierWrapper,
    mars_lp_staking_contract: String,
    contract_addr: Addr,
) -> mars_periphery::lp_staking::StakerInfoResponse {
    let pending_rewards: mars_periphery::lp_staking::StakerInfoResponse = querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: mars_lp_staking_contract,
            msg: to_binary(&mars_periphery::lp_staking::QueryMsg::StakerInfo {
                staker: contract_addr.to_string(),
                timestamp: None,
            })
            .unwrap(),
        }))
        .unwrap();
    pending_rewards
}

//----------------------------------------------------------------------------------------
// HELPERS :: BUILD COSMOS MSG
//----------------------------------------------------------------------------------------

/// @dev Returns CosmosMsg struct to stake LP Tokens with the MARS LP Staking contract
/// @param amount : LP tokens to stake
pub fn build_stake_with_mars_staking_contract_msg(
    config: Config,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    let stake_msg = to_binary(&mars_periphery::lp_staking::Cw20HookMsg::Bond {})?;
    build_send_cw20_token_msg(
        config
            .mars_lp_staking_contract
            .expect("LP Staking address not set")
            .to_string(),
        config
            .lp_token_address
            .expect("LP Token address not set")
            .to_string(),
        amount,
        stake_msg,
    )
}

/// @dev Returns CosmosMsg struct to unstake LP Tokens from MARS LP Staking contract
/// @param config : Configuration
/// @param amount : LP tokens to unstake
/// @param claim_rewards : Boolean value indicating is Rewards are to be claimed or not
pub fn build_unstake_from_mars_staking_contract_msg(
    mars_lp_staking_contract: String,
    amount: Uint128,
    claim_rewards: bool,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: mars_lp_staking_contract,
        msg: to_binary(&mars_periphery::lp_staking::ExecuteMsg::Unbond {
            amount,
            withdraw_pending_reward: Some(claim_rewards),
        })?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to claim MARS from MARS LP Staking contract
/// @param mars_lp_staking_contract : Mars LP Staking contract
pub fn build_claim_rewards_from_mars_staking_contract_msg(
    mars_lp_staking_contract: String,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: mars_lp_staking_contract,
        msg: to_binary(&mars_periphery::lp_staking::ExecuteMsg::Claim {})?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to stake LP Tokens with the Generator contract
/// @param config : Configuration
/// @param amount : LP tokens to stake
pub fn build_stake_with_generator_msg(config: Config, amount: Uint128) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config
            .lp_token_address
            .expect("LP Token address not set")
            .to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Send {
            contract: config.generator_contract.to_string(),
            msg: to_binary(&astroport::generator::Cw20HookMsg::Deposit {})?,
            amount,
        })?,
        funds: vec![],
    }))
}

/// @dev Returns CosmosMsg struct to unstake LP Tokens from the Generator contract
/// @param lp_shares_to_unstake : LP tokens to be unstaked from generator  
pub fn build_unstake_from_generator_msg(
    config: &Config,
    lp_shares_to_withdraw: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.generator_contract.to_string(),
        msg: to_binary(&astroport::generator::ExecuteMsg::Withdraw {
            lp_token: config.lp_token_address.clone().expect("LP Token not set"),
            amount: lp_shares_to_withdraw,
        })?,
        funds: vec![],
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to facilitate liquidity provision to the Astroport LP Pool
/// @param slippage_tolerance : Optional slippage parameter
fn build_provide_liquidity_to_lp_pool_msg(
    deps: Deps,
    config: Config,
    state: &State,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<CosmosMsg> {
    let mars = Asset {
        amount: state.total_mars_deposited,
        info: AssetInfo::Token {
            contract_addr: config.mars_token_address.clone(),
        },
    };

    let mut ust = Asset {
        amount: state.total_ust_deposited,
        info: AssetInfo::NativeToken {
            denom: String::from(UUSD_DENOM),
        },
    };

    // Deduct tax
    ust.amount = ust.amount.checked_sub(ust.compute_tax(&deps.querier)?)?;

    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config
            .astroport_lp_pool
            .expect("Mars-uust LP pool not set")
            .to_string(),
        funds: vec![Coin {
            denom: String::from(UUSD_DENOM),
            amount: ust.amount,
        }],
        msg: to_binary(&astroport::pair::ExecuteMsg::ProvideLiquidity {
            assets: [ust, mars],
            slippage_tolerance,
            auto_stake: Some(false),
            receiver: None,
        })?,
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to activate MARS tokens claim from the lockdrop contract
/// @param lockdrop_contract_address : Lockdrop contract address
fn build_activate_claims_lockdrop_msg(lockdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lockdrop_contract_address.to_string(),
        msg: to_binary(&LockdropEnableClaims {})?,
        funds: vec![],
    }))
}

/// @dev Helper function. Returns CosmosMsg struct to activate MARS tokens claim from the airdrop contract
/// @param airdrop_contract_address : Airdrop contract address
fn build_activate_claims_airdrop_msg(airdrop_contract_address: Addr) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: airdrop_contract_address.to_string(),
        msg: to_binary(&AirdropEnableClaims {})?,
        funds: vec![],
    }))
}
