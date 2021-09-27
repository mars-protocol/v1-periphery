use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use terraswap::asset::AssetInfo;

use crate::error::ContractError;
use crate::state::{Config, Cooldown, CONFIG, COOLDOWNS};

use mars::address_provider;
use mars::address_provider::msg::MarsContract;
use mars::error::MarsError;
use mars::helpers::{cw20_get_balance, cw20_get_total_supply, option_string_to_addr, zero_address};
use mars::staking::msg::{
    ConfigResponse, CooldownResponse, CreateOrUpdateConfig, ExecuteMsg, InstantiateMsg, QueryMsg,
    ReceiveMsg,
};
use mars::swapping::execute_swap;

// INIT

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    // Destructuring a struct’s fields into separate variables in order to force
    // compile error if we add more params
    let CreateOrUpdateConfig {
        owner,
        address_provider_address,
        terraswap_factory_address,
        terraswap_max_spread,
        cooldown_duration,
        unstake_window,
    } = msg.config;

    // All fields should be available
    let available = owner.is_some()
        && address_provider_address.is_some()
        && terraswap_factory_address.is_some()
        && terraswap_max_spread.is_some()
        && cooldown_duration.is_some()
        && unstake_window.is_some();

    if !available {
        return Err(MarsError::InstantiateParamsUnavailable {}.into());
    };

    // Initialize config
    let config = Config {
        owner: option_string_to_addr(deps.api, owner, zero_address())?,
        address_provider_address: option_string_to_addr(
            deps.api,
            address_provider_address,
            zero_address(),
        )?,
        terraswap_factory_address: option_string_to_addr(
            deps.api,
            terraswap_factory_address,
            zero_address(),
        )?,
        terraswap_max_spread: terraswap_max_spread.unwrap(),
        cooldown_duration: cooldown_duration.unwrap(),
        unstake_window: unstake_window.unwrap(),
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

// HANDLERS

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { config } => Ok(execute_update_config(deps, info, config)?),
        ExecuteMsg::Receive(cw20_msg) => Ok(execute_receive_cw20(deps, env, info, cw20_msg)?),
        ExecuteMsg::Cooldown {} => Ok(execute_cooldown(deps, env, info)?),
        ExecuteMsg::ExecuteCosmosMsg(cosmos_msg) => {
            Ok(execute_execute_cosmos_msg(deps, info, cosmos_msg)?)
        }
        ExecuteMsg::SwapAssetToUusd {
            offer_asset_info,
            amount,
        } => Ok(execute_swap_asset_to_uusd(
            deps,
            env,
            offer_asset_info,
            amount,
        )?),
        ExecuteMsg::SwapUusdToMars { amount } => Ok(execute_swap_uusd_to_mars(deps, env, amount)?),
    }
}

/// Update config
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: CreateOrUpdateConfig,
) -> Result<Response, MarsError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {});
    }

    // Destructuring a struct’s fields into separate variables in order to force
    // compile error if we add more params
    let CreateOrUpdateConfig {
        owner,
        address_provider_address,
        terraswap_factory_address,
        terraswap_max_spread,
        cooldown_duration,
        unstake_window,
    } = new_config;

    // Update config
    config.owner = option_string_to_addr(deps.api, owner, config.owner)?;
    config.address_provider_address = option_string_to_addr(
        deps.api,
        address_provider_address,
        config.address_provider_address,
    )?;
    config.terraswap_factory_address = option_string_to_addr(
        deps.api,
        terraswap_factory_address,
        config.terraswap_factory_address,
    )?;
    config.terraswap_max_spread = terraswap_max_spread.unwrap_or(config.terraswap_max_spread);
    config.cooldown_duration = cooldown_duration.unwrap_or(config.cooldown_duration);
    config.unstake_window = unstake_window.unwrap_or(config.unstake_window);

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

/// cw20 receive implementation
pub fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_binary(&cw20_msg.msg)? {
        ReceiveMsg::Stake { recipient } => {
            execute_stake(deps, env, info, cw20_msg.sender, recipient, cw20_msg.amount)
        }
        ReceiveMsg::Unstake { recipient } => {
            execute_unstake(deps, env, info, cw20_msg.sender, recipient, cw20_msg.amount)
        }
    }
}

/// Mint xMars tokens to staker
pub fn execute_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staker: String,
    option_recipient: Option<String>,
    stake_amount: Uint128,
) -> Result<Response, ContractError> {
    // check stake is valid
    let config = CONFIG.load(deps.storage)?;
    let (mars_token_address, xmars_token_address) = get_token_addresses(&deps, &config)?;

    // Has to send Mars tokens
    if info.sender != mars_token_address {
        return Err(MarsError::Unauthorized {}.into());
    }
    if stake_amount == Uint128::zero() {
        return Err(ContractError::StakeAmountZero {});
    }

    let total_mars_in_staking_contract =
        cw20_get_balance(&deps.querier, mars_token_address, env.contract.address)?;
    // Mars amount needs to be before the stake transaction (which is already in the staking contract's
    // balance so it needs to be deducted)
    let net_total_mars_in_staking_contract =
        total_mars_in_staking_contract.checked_sub(stake_amount)?;

    let total_xmars_supply = cw20_get_total_supply(&deps.querier, xmars_token_address.clone())?;

    let mint_amount = if net_total_mars_in_staking_contract == Uint128::zero()
        || total_xmars_supply == Uint128::zero()
    {
        stake_amount
    } else {
        stake_amount.multiply_ratio(total_xmars_supply, net_total_mars_in_staking_contract)
    };

    let recipient = option_recipient.unwrap_or_else(|| staker.clone());

    let res = Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: xmars_token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: recipient.clone(),
                amount: mint_amount,
            })?,
        }))
        .add_attribute("action", "stake")
        .add_attribute("staker", staker)
        .add_attribute("recipient", recipient)
        .add_attribute("mars_staked", stake_amount)
        .add_attribute("xmars_minted", mint_amount);

    Ok(res)
}

/// Burn xMars tokens and send corresponding Mars
pub fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staker: String,
    option_recipient: Option<String>,
    burn_amount: Uint128,
) -> Result<Response, ContractError> {
    // check if unstake is valid
    let config = CONFIG.load(deps.storage)?;
    let (mars_token_address, xmars_token_address) = get_token_addresses(&deps, &config)?;
    if info.sender != xmars_token_address {
        return Err(MarsError::Unauthorized {}.into());
    }
    if burn_amount == Uint128::zero() {
        return Err(ContractError::UnstakeAmountZero {});
    }

    // check valid cooldown
    let staker_addr = deps.api.addr_validate(&staker)?;

    match COOLDOWNS.may_load(deps.storage, &staker_addr)? {
        Some(mut cooldown) => {
            if burn_amount > cooldown.amount {
                return Err(ContractError::UnstakeAmountTooLarge {});
            }
            if env.block.time.seconds() < cooldown.timestamp + config.cooldown_duration {
                return Err(ContractError::UnstakeCooldownNotFinished {});
            }
            if env.block.time.seconds()
                > cooldown.timestamp + config.cooldown_duration + config.unstake_window
            {
                return Err(ContractError::UnstakeCooldownExpired {});
            }

            if burn_amount == cooldown.amount {
                COOLDOWNS.remove(deps.storage, &staker_addr);
            } else {
                cooldown.amount = cooldown.amount.checked_sub(burn_amount)?;
                COOLDOWNS.save(deps.storage, &staker_addr, &cooldown)?;
            }
        }

        None => {
            return Err(ContractError::UnstakeNoCooldown {});
        }
    };

    let total_mars_in_staking_contract = cw20_get_balance(
        &deps.querier,
        mars_token_address.clone(),
        env.contract.address,
    )?;

    let total_xmars_supply = cw20_get_total_supply(&deps.querier, xmars_token_address.clone())?;

    let unstake_amount =
        burn_amount.multiply_ratio(total_mars_in_staking_contract, total_xmars_supply);

    let recipient = option_recipient.unwrap_or_else(|| staker.clone());

    let res = Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: xmars_token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Burn {
                amount: burn_amount,
            })?,
        }))
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mars_token_address.to_string(),
            funds: vec![],
            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                recipient: recipient.clone(),
                amount: unstake_amount,
            })?,
        }))
        .add_attribute("action", "unstake")
        .add_attribute("staker", staker)
        .add_attribute("recipient", recipient)
        .add_attribute("mars_unstaked", unstake_amount)
        .add_attribute("xmars_burned", burn_amount);
    Ok(res)
}

/// Handles cooldown. if staking non zero amount, activates a cooldown for that amount.
/// If a cooldown exists and amount has changed it computes the weighted average
/// for the cooldown
pub fn execute_cooldown(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let xmars_token_address = address_provider::helpers::query_address(
        &deps.querier,
        config.address_provider_address,
        MarsContract::XMarsToken,
    )?;

    // get total xMars in contract before the stake transaction
    let xmars_balance = cw20_get_balance(&deps.querier, xmars_token_address, info.sender.clone())?;

    if xmars_balance.is_zero() {
        return Err(MarsError::Unauthorized {}.into());
    }

    // compute new cooldown timestamp
    let new_cooldown_timestamp = match COOLDOWNS.may_load(deps.storage, &info.sender)? {
        Some(cooldown) => {
            let minimal_valid_cooldown_timestamp =
                env.block.time.seconds() - config.cooldown_duration - config.unstake_window;

            if cooldown.timestamp < minimal_valid_cooldown_timestamp {
                env.block.time.seconds()
            } else {
                let mut extra_amount: u128 = 0;
                if xmars_balance > cooldown.amount {
                    extra_amount = (xmars_balance.checked_sub(cooldown.amount)?).u128();
                };

                (((cooldown.timestamp as u128) * cooldown.amount.u128()
                    + (env.block.time.seconds() as u128) * extra_amount)
                    / (cooldown.amount.u128() + extra_amount)) as u64
            }
        }

        None => env.block.time.seconds(),
    };

    COOLDOWNS.save(
        deps.storage,
        &info.sender,
        &Cooldown {
            amount: xmars_balance,
            timestamp: new_cooldown_timestamp,
        },
    )?;

    let res = Response::new()
        .add_attribute("action", "cooldown")
        .add_attribute("user", info.sender)
        .add_attribute("cooldown_amount", xmars_balance.to_string())
        .add_attribute("cooldown_timestamp", new_cooldown_timestamp.to_string());
    Ok(res)
}

/// Execute Cosmos message
pub fn execute_execute_cosmos_msg(
    deps: DepsMut,
    info: MessageInfo,
    msg: CosmosMsg,
) -> Result<Response, MarsError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {});
    }

    let res = Response::new()
        .add_message(msg)
        .add_attribute("action", "execute_cosmos_msg");
    Ok(res)
}

/// Swap any asset on the contract to uusd
pub fn execute_swap_asset_to_uusd(
    deps: DepsMut,
    env: Env,
    offer_asset_info: AssetInfo,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // throw error if the user tries to swap Mars
    let mars_token_address = address_provider::helpers::query_address(
        &deps.querier,
        config.address_provider_address,
        MarsContract::MarsToken,
    )?;

    if let AssetInfo::Token { contract_addr } = offer_asset_info.clone() {
        if contract_addr == mars_token_address {
            return Err(ContractError::MarsCannotSwap {});
        }
    }

    let ask_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let terraswap_max_spread = Some(config.terraswap_max_spread);

    Ok(execute_swap(
        deps,
        env,
        offer_asset_info,
        ask_asset_info,
        amount,
        config.terraswap_factory_address,
        terraswap_max_spread,
    )?)
}

/// Swap uusd on the contract to Mars
pub fn execute_swap_uusd_to_mars(
    deps: DepsMut,
    env: Env,
    amount: Option<Uint128>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let offer_asset_info = AssetInfo::NativeToken {
        denom: "uusd".to_string(),
    };

    let mars_token_address = address_provider::helpers::query_address(
        &deps.querier,
        config.address_provider_address,
        MarsContract::MarsToken,
    )?;

    let ask_asset_info = AssetInfo::Token {
        contract_addr: mars_token_address.to_string(),
    };

    let terraswap_max_spread = Some(config.terraswap_max_spread);

    execute_swap(
        deps,
        env,
        offer_asset_info,
        ask_asset_info,
        amount,
        config.terraswap_factory_address,
        terraswap_max_spread,
    )
}

// QUERIES

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Cooldown { sender_address } => to_binary(&query_cooldown(deps, sender_address)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        address_provider_address: config.address_provider_address.to_string(),
        terraswap_max_spread: config.terraswap_max_spread,
        cooldown_duration: config.cooldown_duration,
        unstake_window: config.unstake_window,
    })
}

fn query_cooldown(deps: Deps, user_address: String) -> StdResult<CooldownResponse> {
    let cooldown = COOLDOWNS.may_load(deps.storage, &deps.api.addr_validate(&user_address)?)?;

    match cooldown {
        Some(result) => Ok(CooldownResponse {
            timestamp: result.timestamp,
            amount: result.amount,
        }),
        None => Result::Err(StdError::not_found("No cooldown found")),
    }
}

// HELPERS

/// Gets mars and xmars token addresses from address provider and returns them in a tuple.
fn get_token_addresses(deps: &DepsMut, config: &Config) -> Result<(Addr, Addr), ContractError> {
    let mut addresses_query = address_provider::helpers::query_addresses(
        &deps.querier,
        config.address_provider_address.clone(),
        vec![MarsContract::MarsToken, MarsContract::XMarsToken],
    )?;
    let xmars_token_address = addresses_query.pop().unwrap();
    let mars_token_address = addresses_query.pop().unwrap();

    Ok((mars_token_address, xmars_token_address))
}
