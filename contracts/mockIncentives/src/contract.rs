use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Order, OverflowError, OverflowOperation, QueryRequest, Response, StdError,
    StdResult, Uint128, WasmMsg, WasmQuery,
};

use crate::error::ContractError;
use crate::state::{
    AssetIncentive, Config, ASSET_INCENTIVES, CONFIG, USER_ASSET_INDICES, USER_UNCLAIMED_REWARDS,
};
use mars::address_provider::{helpers::query_addresses, msg::MarsContract};
use mars::error::MarsError;
use mars::helpers::option_string_to_addr;
use mars::incentives::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg};

// INIT

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // initialize Config
    let config = Config {
        owner: deps.api.addr_validate(&msg.owner)?,
        address_provider_address: deps.api.addr_validate(&msg.address_provider_address)?,
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
        ExecuteMsg::SetAssetIncentive {
            ma_token_address,
            emission_per_second,
        } => execute_set_asset_incentive(deps, env, info, ma_token_address, emission_per_second),
        ExecuteMsg::BalanceChange {
            user_address,
            user_balance_before,
            total_supply_before,
        } => execute_balance_change(
            deps,
            env,
            info,
            user_address,
            user_balance_before,
            total_supply_before,
        ),
        ExecuteMsg::ClaimRewards {} => execute_claim_rewards(deps, env, info),
        ExecuteMsg::UpdateConfig {
            owner,
            address_provider_address,
        } => Ok(execute_update_config(
            deps,
            env,
            info,
            owner,
            address_provider_address,
        )?),
        ExecuteMsg::ExecuteCosmosMsg(cosmos_msg) => {
            Ok(execute_execute_cosmos_msg(deps, env, info, cosmos_msg)?)
        }
    }
}

pub fn execute_set_asset_incentive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ma_token_address: String,
    emission_per_second: Uint128,
) -> Result<Response, ContractError> {
    // only owner can call this
    let config = CONFIG.load(deps.storage)?;
    let owner = config.owner;
    if info.sender != owner {
        return Err(MarsError::Unauthorized {}.into());
    }

    let ma_asset_address = deps.api.addr_validate(&ma_token_address)?;

    let new_asset_incentive = match ASSET_INCENTIVES.may_load(deps.storage, &ma_asset_address)? {
        Some(mut asset_incentive) => {
            // Update index up to now
            let total_supply =
                mars::helpers::cw20_get_total_supply(&deps.querier, ma_asset_address.clone())?;
            asset_incentive_update_index(
                &mut asset_incentive,
                total_supply,
                env.block.time.seconds(),
            )?;

            // Set new emission
            asset_incentive.emission_per_second = emission_per_second;

            asset_incentive
        }
        None => AssetIncentive {
            emission_per_second,
            index: Decimal::zero(),
            last_updated: env.block.time.seconds(),
        },
    };

    ASSET_INCENTIVES.save(deps.storage, &ma_asset_address, &new_asset_incentive)?;

    let response = Response::new().add_attributes(vec![
        attr("action", "set_asset_incentives"),
        attr("ma_asset", ma_token_address),
        attr("emission_per_second", emission_per_second),
    ]);
    Ok(response)
}

pub fn execute_balance_change(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_address_: String,
    user_balance_before: Uint128,
    total_supply_before: Uint128,
) -> Result<Response, ContractError> {
    let user_address = deps.api.addr_validate(&user_address_)?;
    let ma_token_address = info.sender;
    let mut asset_incentive = match ASSET_INCENTIVES.may_load(deps.storage, &ma_token_address)? {
        // If there are no incentives,
        // an empty successful response is returned as the
        // success of the call is needed for the call that triggered the change to
        // succeed and be persisted to state.
        None => return Ok(Response::default()),

        Some(ai) => ai,
    };

    asset_incentive_update_index(
        &mut asset_incentive,
        total_supply_before,
        env.block.time.seconds(),
    )?;
    ASSET_INCENTIVES.save(deps.storage, &ma_token_address, &asset_incentive)?;

    // Check if user has accumulated uncomputed rewards (which means index is not up to date)
    let user_asset_index_key = USER_ASSET_INDICES.key((&user_address, &ma_token_address));

    let user_asset_index = user_asset_index_key
        .may_load(deps.storage)?
        .unwrap_or_else(Decimal::zero);

    let mut accrued_rewards = Uint128::zero();

    if user_asset_index != asset_incentive.index {
        // Compute user accrued rewards and update state
        accrued_rewards = user_compute_accrued_rewards(
            user_balance_before,
            user_asset_index,
            asset_incentive.index,
        )?;

        // Store user accrued rewards as unclaimed
        if !accrued_rewards.is_zero() {
            USER_UNCLAIMED_REWARDS.update(
                deps.storage,
                &user_address,
                |ur: Option<Uint128>| -> StdResult<Uint128> {
                    match ur {
                        Some(unclaimed_rewards) => Ok(unclaimed_rewards + accrued_rewards),
                        None => Ok(accrued_rewards),
                    }
                },
            )?;
        }

        user_asset_index_key.save(deps.storage, &asset_incentive.index)?;
    } else if user_asset_index.is_zero() {
        // This ensures asset counts for checking rewards. Handles the edge case of user being the
        // first depositor and the incentives being initialized before any deposits happened before.
        // Both indices will be 0 but we should track the asset for user to claim rewards.
        user_asset_index_key.save(deps.storage, &asset_incentive.index)?;
    }

    let response = Response::new().add_attributes(vec![
        attr("action", "balance_change"),
        attr("ma_asset", ma_token_address),
        attr("user", user_address),
        attr("rewards_accrued", accrued_rewards),
        attr("asset_index", asset_incentive.index.to_string()),
    ]);

    Ok(response)
}

pub fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let user_address = info.sender;
    let (total_unclaimed_rewards, asset_incentive_statuses) =
        compute_user_unclaimed_rewards(deps.as_ref(), &env, &user_address)?;

    // Commit updated asset_incentives and user indexes
    for asset_incentive_status in asset_incentive_statuses {
        let asset_incentive_updated = asset_incentive_status.asset_incentive_updated;

        ASSET_INCENTIVES.save(
            deps.storage,
            &asset_incentive_status.ma_token_address,
            &asset_incentive_updated,
        )?;

        if asset_incentive_updated.index != asset_incentive_status.user_index_current {
            USER_ASSET_INDICES.save(
                deps.storage,
                (&user_address, &asset_incentive_status.ma_token_address),
                &asset_incentive_updated.index,
            )?
        }
    }

    // clear unclaimed rewards
    USER_UNCLAIMED_REWARDS.save(deps.storage, &user_address, &Uint128::zero())?;

    let mut response = Response::new();
    if total_unclaimed_rewards > Uint128::zero() {
        // Build message to stake mars and send resulting xmars to the user
        let config = CONFIG.load(deps.storage)?;
        let mars_contracts = vec![MarsContract::MarsToken, MarsContract::Staking];
        let mut addresses_query = query_addresses(
            &deps.querier,
            config.address_provider_address,
            mars_contracts,
        )?;
        let staking_address = addresses_query.pop().unwrap();
        let mars_token_address = addresses_query.pop().unwrap();

        response = response.add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: mars_token_address.to_string(),
            msg: to_binary(&cw20::Cw20ExecuteMsg::Send {
                contract: staking_address.to_string(),
                amount: total_unclaimed_rewards,
                msg: to_binary(&mars::staking::msg::ReceiveMsg::Stake {
                    recipient: Some(user_address.to_string()),
                })?,
            })?,
            funds: vec![],
        }));
    };

    response = response.add_attributes(vec![
        attr("action", "claim_rewards"),
        attr("user", user_address),
        attr("mars_staked_as_rewards", total_unclaimed_rewards),
    ]);

    Ok(response)
}

pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    address_provider_address: Option<String>,
) -> Result<Response, MarsError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {});
    };

    config.owner = option_string_to_addr(deps.api, owner, config.owner)?;
    config.address_provider_address = option_string_to_addr(
        deps.api,
        address_provider_address,
        config.address_provider_address,
    )?;

    CONFIG.save(deps.storage, &config)?;

    let response = Response::new().add_attribute("action", "update_config");

    Ok(response)
}

pub fn execute_execute_cosmos_msg(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: CosmosMsg,
) -> Result<Response, MarsError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {});
    }

    let response = Response::new()
        .add_attribute("action", "execute_cosmos_msg")
        .add_message(msg);

    Ok(response)
}

// HELPERS

/// Updates asset incentive index and last updated timestamp by computing
/// how many rewards were accrued since last time updated given incentive's
/// emission per second.
/// Total supply is the total (liquidity) token supply during the period being computed.
/// Note that this method does not commit updates to state as that should be executed by the
/// caller
fn asset_incentive_update_index(
    asset_incentive: &mut AssetIncentive,
    total_supply: Uint128,
    current_block_time: u64,
) -> StdResult<()> {
    if (current_block_time != asset_incentive.last_updated)
        && !total_supply.is_zero()
        && !asset_incentive.emission_per_second.is_zero()
    {
        asset_incentive.index = asset_incentive_compute_index(
            asset_incentive.index,
            asset_incentive.emission_per_second,
            total_supply,
            asset_incentive.last_updated,
            current_block_time,
        )?
    }
    asset_incentive.last_updated = current_block_time;
    Ok(())
}

fn asset_incentive_compute_index(
    previous_index: Decimal,
    emission_per_second: Uint128,
    total_supply: Uint128,
    time_start: u64,
    time_end: u64,
) -> StdResult<Decimal> {
    if time_start > time_end {
        return Err(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            time_start,
            time_end,
        )));
    }
    let seconds_elapsed = time_end - time_start;
    let new_index = previous_index
        + Decimal::from_ratio(
            emission_per_second.u128() * seconds_elapsed as u128,
            total_supply,
        );
    Ok(new_index)
}

/// Computes user accrued rewards using the difference between asset_incentive index and
/// user current index
/// asset_incentives index should be up to date.
fn user_compute_accrued_rewards(
    user_balance: Uint128,
    user_asset_index: Decimal,
    asset_incentive_index: Decimal,
) -> StdResult<Uint128> {
    Ok((user_balance * asset_incentive_index) - (user_balance * user_asset_index))
}

/// Result of querying and updating the status of the user and a give asset incentives in order to
/// compute unclaimed rewards.
struct UserAssetIncentiveStatus {
    /// Address of the ma token that's the incentives target
    ma_token_address: Addr,
    /// Current user index's value on the contract store (not updated by current asset index)
    user_index_current: Decimal,
    /// Asset incentive with values updated to the current block (not neccesarily commited
    /// to storage)
    asset_incentive_updated: AssetIncentive,
}

fn compute_user_unclaimed_rewards(
    deps: Deps,
    env: &Env,
    user_address: &Addr,
) -> StdResult<(Uint128, Vec<UserAssetIncentiveStatus>)> {
    let mut total_unclaimed_rewards = USER_UNCLAIMED_REWARDS
        .may_load(deps.storage, user_address)?
        .unwrap_or_else(Uint128::zero);

    let result_user_asset_indices: StdResult<Vec<_>> = USER_ASSET_INDICES
        .prefix(user_address)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let mut user_asset_incentive_statuses: Vec<UserAssetIncentiveStatus> = vec![];

    for result_kv_pair in result_user_asset_indices? {
        let (ma_token_address_bytes, user_asset_index) = result_kv_pair;

        let ma_token_address = deps
            .api
            .addr_validate(&String::from_utf8(ma_token_address_bytes)?)?;

        // Get asset user balances and total supply
        let balance_and_total_supply: mars::ma_token::msg::BalanceAndTotalSupplyResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: ma_token_address.to_string(),
                msg: to_binary(&mars::ma_token::msg::QueryMsg::BalanceAndTotalSupply {
                    address: user_address.to_string(),
                })?,
            }))?;

        // Get asset pending rewards
        let mut asset_incentive = ASSET_INCENTIVES.load(deps.storage, &ma_token_address)?;

        asset_incentive_update_index(
            &mut asset_incentive,
            balance_and_total_supply.total_supply,
            env.block.time.seconds(),
        )?;

        if user_asset_index != asset_incentive.index {
            // Compute user accrued rewards and update user index
            let asset_accrued_rewards = user_compute_accrued_rewards(
                balance_and_total_supply.balance,
                user_asset_index,
                asset_incentive.index,
            )?;
            total_unclaimed_rewards += asset_accrued_rewards;
        }

        user_asset_incentive_statuses.push(UserAssetIncentiveStatus {
            ma_token_address,
            user_index_current: user_asset_index,
            asset_incentive_updated: asset_incentive,
        });
    }

    Ok((total_unclaimed_rewards, user_asset_incentive_statuses))
}

// QUERIES

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::AssetIncentive { ma_token_address } => {
            to_binary(&query_asset_incentive(deps, ma_token_address)?)
        }
        QueryMsg::UserUnclaimedRewards { user_address } => {
            to_binary(&query_user_unclaimed_rewards(deps, env, user_address)?)
        }
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner,
    })
}

fn query_asset_incentive(
    deps: Deps,
    ma_token_address_unchecked: String,
) -> StdResult<AssetIncentive> {
    let ma_token_address = deps.api.addr_validate(&ma_token_address_unchecked)?;
    let asset_incentive = ASSET_INCENTIVES.load(deps.storage, &ma_token_address)?;
    Ok(asset_incentive)
}

fn query_user_unclaimed_rewards(
    deps: Deps,
    env: Env,
    user_address_unchecked: String,
) -> StdResult<Uint128> {
    let user_address = deps.api.addr_validate(&user_address_unchecked)?;
    let (unclaimed_rewards, _) = compute_user_unclaimed_rewards(deps, &env, &user_address)?;

    Ok(unclaimed_rewards)
}
