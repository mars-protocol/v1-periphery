use std::str;

use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, Event, MessageInfo, Order, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, MinterResponse};
use cw20_base::msg::InstantiateMarketingInfo;
use cw_storage_plus::U32Key;

use mars::address_provider;
use mars::address_provider::msg::MarsContract;
use mars::ma_token;
use mars::red_bank::{
    msg::{
        AmountResponse, CollateralInfo, CollateralResponse, ConfigResponse, CreateOrUpdateConfig,
        DebtInfo, DebtResponse, ExecuteMsg, InitOrUpdateAssetParams, InstantiateMsg, MarketInfo,
        MarketResponse, MarketsListResponse, QueryMsg, ReceiveMsg,
        UncollateralizedLoanLimitResponse, UserPositionResponse,
    },
    UserHealthStatus,
};

use mars::asset::{Asset, AssetType};
use mars::error::MarsError;
use mars::helpers::{cw20_get_balance, cw20_get_symbol, option_string_to_addr, zero_address};
use mars::tax::deduct_tax;

use crate::accounts::get_user_position;
use crate::error::ContractError;
use crate::interest_rates::{
    apply_accumulated_interests, get_descaled_amount, get_scaled_amount, get_updated_borrow_index,
    get_updated_liquidity_index, update_interest_rates,
};
use crate::state::{
    Config, Debt, GlobalState, Market, User, CONFIG, DEBTS, GLOBAL_STATE, MARKETS,
    MARKET_REFERENCES_BY_INDEX, MARKET_REFERENCES_BY_MA_TOKEN, UNCOLLATERALIZED_LOAN_LIMITS, USERS,
};
use mars::math::reverse_decimal;

// INIT

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // Destructuring a struct’s fields into separate variables in order to force
    // compile error if we add more params
    let CreateOrUpdateConfig {
        owner,
        address_provider_address,
        insurance_fund_fee_share,
        treasury_fee_share,
        ma_token_code_id,
        close_factor,
    } = msg.config;

    // All fields should be available
    let available = owner.is_some()
        && address_provider_address.is_some()
        && insurance_fund_fee_share.is_some()
        && treasury_fee_share.is_some()
        && ma_token_code_id.is_some()
        && close_factor.is_some();

    if !available {
        return Err(StdError::generic_err(
            "All params should be available during initialization",
        ));
    };

    let config = Config {
        owner: option_string_to_addr(deps.api, owner, zero_address())?,
        address_provider_address: option_string_to_addr(
            deps.api,
            address_provider_address,
            zero_address(),
        )?,
        ma_token_code_id: ma_token_code_id.unwrap(),
        close_factor: close_factor.unwrap(),
        insurance_fund_fee_share: insurance_fund_fee_share.unwrap(),
        treasury_fee_share: treasury_fee_share.unwrap(),
    };
    config.validate()?;

    CONFIG.save(deps.storage, &config)?;

    GLOBAL_STATE.save(deps.storage, &GlobalState { market_count: 0 })?;

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
        ExecuteMsg::UpdateConfig { config } => execute_update_config(deps, env, info, config),

        ExecuteMsg::Receive(cw20_msg) => execute_receive_cw20(deps, env, info, cw20_msg),

        ExecuteMsg::InitAsset {
            asset,
            asset_params,
        } => execute_init_asset(deps, env, info, asset, asset_params),

        ExecuteMsg::InitAssetTokenCallback { reference } => {
            execute_init_asset_token_callback(deps, env, info, reference)
        }

        ExecuteMsg::UpdateAsset {
            asset,
            asset_params,
        } => execute_update_asset(deps, env, info, asset, asset_params),

        ExecuteMsg::DepositNative { denom } => {
            let deposit_amount = get_denom_amount_from_coins(&info.funds, &denom);
            let depositor_address = info.sender.clone();
            execute_deposit(
                deps,
                env,
                info,
                depositor_address,
                denom.as_bytes(),
                denom.as_str(),
                deposit_amount,
            )
        }

        ExecuteMsg::Borrow { asset, amount } => execute_borrow(deps, env, info, asset, amount),

        ExecuteMsg::RepayNative { denom } => {
            let repayer_address = info.sender.clone();
            let repay_amount = get_denom_amount_from_coins(&info.funds, &denom);
            execute_repay(
                deps,
                env,
                info,
                repayer_address,
                denom.as_bytes(),
                denom.as_str(),
                repay_amount,
                AssetType::Native,
            )
        }

        ExecuteMsg::LiquidateNative {
            collateral_asset,
            debt_asset_denom,
            user_address,
            receive_ma_token,
        } => {
            let sender = info.sender.clone();
            let user_addr = deps.api.addr_validate(&user_address)?;
            let sent_debt_asset_amount =
                get_denom_amount_from_coins(&info.funds, &debt_asset_denom);
            execute_liquidate(
                deps,
                env,
                info,
                sender,
                collateral_asset,
                Asset::Native {
                    denom: debt_asset_denom,
                },
                user_addr,
                sent_debt_asset_amount,
                receive_ma_token,
            )
        }

        ExecuteMsg::FinalizeLiquidityTokenTransfer {
            sender_address,
            recipient_address,
            sender_previous_balance,
            recipient_previous_balance,
            amount,
        } => execute_finalize_liquidity_token_transfer(
            deps,
            env,
            info,
            sender_address,
            recipient_address,
            sender_previous_balance,
            recipient_previous_balance,
            amount,
        ),

        ExecuteMsg::UpdateUncollateralizedLoanLimit {
            user_address,
            asset,
            new_limit,
        } => {
            let user_addr = deps.api.addr_validate(&user_address)?;
            execute_update_uncollateralized_loan_limit(deps, env, info, user_addr, asset, new_limit)
        }
        ExecuteMsg::UpdateUserCollateralAssetStatus { asset, enable } => {
            execute_update_user_collateral_asset_status(deps, env, info, asset, enable)
        }

        ExecuteMsg::DistributeProtocolIncome { asset, amount } => {
            execute_distribute_protocol_income(deps, env, info, asset, amount)
        }

        ExecuteMsg::Withdraw { asset, amount } => execute_withdraw(deps, env, info, asset, amount),
    }
}

/// Update config
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: CreateOrUpdateConfig,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {}.into());
    }

    // Destructuring a struct’s fields into separate variables in order to force
    // compile error if we add more params
    let CreateOrUpdateConfig {
        owner,
        address_provider_address,
        insurance_fund_fee_share,
        treasury_fee_share,
        ma_token_code_id,
        close_factor,
    } = new_config;

    // Update config
    config.owner = option_string_to_addr(deps.api, owner, config.owner)?;
    config.address_provider_address = option_string_to_addr(
        deps.api,
        address_provider_address,
        config.address_provider_address,
    )?;
    config.ma_token_code_id = ma_token_code_id.unwrap_or(config.ma_token_code_id);
    config.close_factor = close_factor.unwrap_or(config.close_factor);
    config.insurance_fund_fee_share =
        insurance_fund_fee_share.unwrap_or(config.insurance_fund_fee_share);
    config.treasury_fee_share = treasury_fee_share.unwrap_or(config.treasury_fee_share);

    // Validate config
    config.validate()?;

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
        ReceiveMsg::DepositCw20 {} => {
            let depositor_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            let token_contract_address = info.sender.clone();
            execute_deposit(
                deps,
                env,
                info,
                depositor_addr,
                token_contract_address.as_bytes(),
                token_contract_address.as_str(),
                cw20_msg.amount,
            )
        }
        ReceiveMsg::RepayCw20 {} => {
            let repayer_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            let token_contract_address = info.sender.clone();
            execute_repay(
                deps,
                env,
                info,
                repayer_addr,
                token_contract_address.as_bytes(),
                token_contract_address.as_str(),
                cw20_msg.amount,
                AssetType::Cw20,
            )
        }
        ReceiveMsg::LiquidateCw20 {
            collateral_asset,
            user_address,
            receive_ma_token,
        } => {
            let debt_asset_addr = info.sender.clone();
            let liquidator_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            let user_addr = deps.api.addr_validate(&user_address)?;
            execute_liquidate(
                deps,
                env,
                info,
                liquidator_addr,
                collateral_asset,
                Asset::Cw20 {
                    contract_addr: debt_asset_addr.to_string(),
                },
                user_addr,
                cw20_msg.amount,
                receive_ma_token,
            )
        }
    }
}

/// Burns sent maAsset in exchange of underlying asset
pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let withdrawer_addr = info.sender;

    let (asset_label, asset_reference, _asset_type) = asset.get_attributes();
    let mut market = MARKETS.load(deps.storage, asset_reference.as_slice())?;

    let asset_ma_addr = market.ma_token_address.clone();
    let withdrawer_balance_scaled =
        cw20_get_balance(&deps.querier, asset_ma_addr, withdrawer_addr.clone())?;

    if withdrawer_balance_scaled.is_zero() {
        return Err(StdError::generic_err(
            format!("User has no balance (asset: {})", asset_label,),
        )
        .into());
    }

    // Check user has sufficient balance to send back
    let (withdraw_amount, withdraw_amount_scaled) = match amount {
        Some(amount) => {
            let amount_scaled = get_scaled_amount(
                amount,
                get_updated_liquidity_index(&market, env.block.time.seconds()),
            );
            if amount_scaled.is_zero() || amount_scaled > withdrawer_balance_scaled {
                return Err(StdError::generic_err(format!(
                    "Withdraw amount must be greater than 0 and less or equal user balance (asset: {})",
                    asset_label,
                )).into());
            };
            (amount, amount_scaled)
        }
        None => {
            // NOTE: We prefer to just do one multiplication equation instead of two: division and multiplication.
            // This helps to avoid rounding errors if we want to be sure in burning total balance.
            let withdrawer_balance = get_descaled_amount(
                withdrawer_balance_scaled,
                get_updated_liquidity_index(&market, env.block.time.seconds()),
            );
            (withdrawer_balance, withdrawer_balance_scaled)
        }
    };

    let mut withdrawer = USERS.load(deps.storage, &withdrawer_addr)?;
    let asset_as_collateral = get_bit(withdrawer.collateral_assets, market.index)?;
    let user_is_borrowing = !withdrawer.borrowed_assets.is_zero();

    // if asset is used as collateral and user is borrowing we need to validate health factor after withdraw,
    // otherwise no reasons to block the withdraw
    if asset_as_collateral && user_is_borrowing {
        let global_state = GLOBAL_STATE.load(deps.storage)?;
        let config = CONFIG.load(deps.storage)?;

        let oracle_address = address_provider::helpers::query_address(
            &deps.querier,
            config.address_provider_address,
            MarsContract::Oracle,
        )?;

        let user_position = get_user_position(
            deps.as_ref(),
            env.block.time.seconds(),
            &withdrawer_addr,
            oracle_address,
            &withdrawer,
            global_state.market_count,
        )?;

        let withdraw_asset_price =
            user_position.get_asset_price(asset_reference.as_slice(), &asset_label)?;

        let withdraw_amount_in_uusd = withdraw_amount * withdraw_asset_price;

        let health_factor_after_withdraw = Decimal::from_ratio(
            user_position.weighted_maintenance_margin_in_uusd
                - (withdraw_amount_in_uusd * market.maintenance_margin),
            user_position.total_collateralized_debt_in_uusd,
        );
        if health_factor_after_withdraw < Decimal::one() {
            return Err(StdError::generic_err(
                "User's health factor can't be less than 1 after withdraw",
            )
            .into());
        }
    }

    let mut events = vec![];
    // if amount to withdraw equals the user's balance then unset collateral bit
    if asset_as_collateral && withdraw_amount_scaled == withdrawer_balance_scaled {
        unset_bit(&mut withdrawer.collateral_assets, market.index)?;
        USERS.save(deps.storage, &withdrawer_addr, &withdrawer)?;
        events.push(build_collateral_position_changed_event(
            asset_label.as_str(),
            false,
            withdrawer_addr.to_string(),
        ));
    }

    apply_accumulated_interests(&env, &mut market);
    update_interest_rates(
        &deps,
        &env,
        asset_reference.as_slice(),
        &mut market,
        withdraw_amount,
    )?;
    MARKETS.save(deps.storage, asset_reference.as_slice(), &market)?;

    let burn_ma_tokens_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: market.ma_token_address.to_string(),
        msg: to_binary(&ma_token::msg::ExecuteMsg::Burn {
            user: withdrawer_addr.to_string(),
            amount: withdraw_amount_scaled,
        })?,
        funds: vec![],
    });

    let send_underlying_asset_msg = build_send_asset_msg(
        deps.as_ref(),
        env.contract.address,
        withdrawer_addr.clone(),
        asset,
        withdraw_amount,
    )?;

    events.push(build_interests_updated_event(asset_label.as_str(), &market));

    let res = Response::new()
        .add_attribute("action", "withdraw")
        .add_attribute("market", asset_label.as_str())
        .add_attribute("user", withdrawer_addr.as_str())
        .add_attribute("burn_amount", withdraw_amount_scaled)
        .add_attribute("withdraw_amount", withdraw_amount)
        .add_message(burn_ma_tokens_msg)
        .add_message(send_underlying_asset_msg)
        .add_events(events);
    Ok(res)
}

/// Initialize asset if not exist.
/// Initialization requires that all params are provided and there is no asset in state.
pub fn execute_init_asset(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
    asset_params: InitOrUpdateAssetParams,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {}.into());
    }

    let mut money_market = GLOBAL_STATE.load(deps.storage)?;

    let (asset_label, asset_reference, asset_type) = asset.get_attributes();
    let market_option = MARKETS.may_load(deps.storage, asset_reference.as_slice())?;
    match market_option {
        None => {
            let market_idx = money_market.market_count;
            let new_market = Market::create(env.block.time, market_idx, asset_type, asset_params)?;

            // Save new market
            MARKETS.save(deps.storage, asset_reference.as_slice(), &new_market)?;

            // Save index to reference mapping
            MARKET_REFERENCES_BY_INDEX.save(
                deps.storage,
                U32Key::new(market_idx),
                &asset_reference.to_vec(),
            )?;

            // Increment market count
            money_market.market_count += 1;
            GLOBAL_STATE.save(deps.storage, &money_market)?;

            let symbol = match asset {
                Asset::Native { denom } => denom,
                Asset::Cw20 { contract_addr } => {
                    let contract_addr = deps.api.addr_validate(&contract_addr)?;
                    cw20_get_symbol(&deps.querier, contract_addr)?
                }
            };

            // Prepare response, should instantiate an maToken
            // and use the Register hook.
            // A new maToken should be created which callbacks this contract in order to be registered.
            let mut addresses_query = address_provider::helpers::query_addresses(
                &deps.querier,
                config.address_provider_address,
                vec![MarsContract::Incentives, MarsContract::ProtocolAdmin],
            )?;

            let protocol_admin_address = addresses_query.pop().unwrap();
            let incentives_address = addresses_query.pop().unwrap();

            let res = Response::new()
                .add_attribute("action", "init_asset")
                .add_attribute("asset", asset_label)
                .add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
                    admin: Some(protocol_admin_address.to_string()),
                    code_id: config.ma_token_code_id,
                    msg: to_binary(&ma_token::msg::InstantiateMsg {
                        name: format!("mars {} liquidity token", symbol),
                        symbol: format!("ma{}", symbol),
                        decimals: 6,
                        initial_balances: vec![],
                        mint: Some(MinterResponse {
                            minter: env.contract.address.to_string(),
                            cap: None,
                        }),
                        marketing: Some(InstantiateMarketingInfo {
                            project: Some(String::from("Mars Protocol")),
                            description: Some(format!(
                                "Interest earning token representing deposits for {}",
                                symbol
                            )),
                            marketing: Some(protocol_admin_address.to_string()),
                            logo: None,
                        }),
                        init_hook: Some(ma_token::msg::InitHook {
                            contract_addr: env.contract.address.to_string(),
                            msg: to_binary(&ExecuteMsg::InitAssetTokenCallback {
                                reference: asset_reference,
                            })?,
                        }),
                        red_bank_address: env.contract.address.to_string(),
                        incentives_address: incentives_address.into(),
                    })?,
                    funds: vec![],
                    label: String::from(""),
                }));
            Ok(res)
        }
        Some(_) => Err(StdError::generic_err("Asset already initialized").into()),
    }
}

pub fn execute_init_asset_token_callback(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    reference: Vec<u8>,
) -> Result<Response, ContractError> {
    let mut market = MARKETS.load(deps.storage, reference.as_slice())?;

    if market.ma_token_address == zero_address() {
        let ma_contract_addr = info.sender;

        market.ma_token_address = ma_contract_addr.clone();
        MARKETS.save(deps.storage, reference.as_slice(), &market)?;

        // save ma token contract to reference mapping
        MARKET_REFERENCES_BY_MA_TOKEN.save(deps.storage, &ma_contract_addr, &reference)?;

        Ok(Response::default())
    } else {
        // Can do this only once
        Err(MarsError::Unauthorized {}.into())
    }
}

/// Update asset with new params.
pub fn execute_update_asset(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
    asset_params: InitOrUpdateAssetParams,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {}.into());
    }

    let (asset_label, asset_reference, _asset_type) = asset.get_attributes();
    let market_option = MARKETS.may_load(deps.storage, asset_reference.as_slice())?;
    match market_option {
        Some(market) => {
            let (updated_market, interest_rates_updated) =
                market.update(&deps, &env, asset_reference.as_slice(), asset_params)?;

            MARKETS.save(deps.storage, asset_reference.as_slice(), &updated_market)?;

            let mut res = Response::new()
                .add_attribute("action", "update_asset")
                .add_attribute("asset", &asset_label);

            if interest_rates_updated {
                res = res.add_event(build_interests_updated_event(&asset_label, &updated_market));
            }

            Ok(res)
        }
        None => Err(StdError::generic_err("Asset not initialized").into()),
    }
}

/// Execute deposits and mint corresponding ma_tokens
pub fn execute_deposit(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    depositor_address: Addr,
    asset_reference: &[u8],
    asset_label: &str,
    deposit_amount: Uint128,
) -> Result<Response, ContractError> {
    let mut market = MARKETS.load(deps.storage, asset_reference)?;

    // Cannot deposit zero amount
    if deposit_amount.is_zero() {
        return Err(StdError::generic_err(format!(
            "Deposit amount must be greater than 0 {}",
            asset_label,
        ))
        .into());
    }

    let mut user = USERS
        .may_load(deps.storage, &depositor_address)?
        .unwrap_or_default();

    let mut events = vec![];
    let has_deposited_asset = get_bit(user.collateral_assets, market.index)?;
    if !has_deposited_asset {
        set_bit(&mut user.collateral_assets, market.index)?;
        USERS.save(deps.storage, &depositor_address, &user)?;
        events.push(build_collateral_position_changed_event(
            asset_label,
            true,
            depositor_address.to_string(),
        ));
    }

    apply_accumulated_interests(&env, &mut market);
    update_interest_rates(&deps, &env, asset_reference, &mut market, Uint128::zero())?;
    MARKETS.save(deps.storage, asset_reference, &market)?;

    if market.liquidity_index.is_zero() {
        return Err(StdError::generic_err("Cannot have 0 as liquidity index").into());
    }
    let mint_amount = get_scaled_amount(
        deposit_amount,
        get_updated_liquidity_index(&market, env.block.time.seconds()),
    );

    events.push(build_interests_updated_event(asset_label, &market));

    let res = Response::new()
        .add_attribute("action", "deposit")
        .add_attribute("market", asset_label)
        .add_attribute("user", depositor_address.as_str())
        .add_attribute("amount", deposit_amount)
        .add_events(events)
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: market.ma_token_address.into(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: depositor_address.into(),
                amount: mint_amount,
            })?,
            funds: vec![],
        }));

    Ok(res)
}

/// Add debt for the borrower and send the borrowed funds
pub fn execute_borrow(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset: Asset,
    borrow_amount: Uint128,
) -> Result<Response, ContractError> {
    let borrower_address = info.sender;
    let (asset_label, asset_reference, asset_type) = asset.get_attributes();

    // Cannot borrow zero amount
    if borrow_amount.is_zero() {
        return Err(StdError::generic_err(format!(
            "Borrow amount must be greater than 0 {}",
            asset_label,
        ))
        .into());
    }

    // Load market and user state
    let global_state = GLOBAL_STATE.load(deps.storage)?;
    let mut borrow_market = match MARKETS.load(deps.storage, asset_reference.as_slice()) {
        Ok(borrow_market) => borrow_market,
        Err(_) => {
            return Err(StdError::generic_err(format!(
                "no borrow market exists with asset reference: {}",
                String::from_utf8(asset_reference).expect("Found invalid UTF-8")
            ))
            .into());
        }
    };
    let uncollateralized_loan_limit = UNCOLLATERALIZED_LOAN_LIMITS
        .may_load(
            deps.storage,
            (asset_reference.as_slice(), &borrower_address),
        )?
        .unwrap_or_else(Uint128::zero);
    let mut user: User = match USERS.may_load(deps.storage, &borrower_address)? {
        Some(user) => user,
        None => {
            if uncollateralized_loan_limit.is_zero() {
                return Err(StdError::generic_err("address has no collateral deposited").into());
            }
            // If User has some uncollateralized_loan_limit, then we don't require an existing debt position and initialize a new one.
            User::default()
        }
    };

    let is_borrowing_asset = get_bit(user.borrowed_assets, borrow_market.index)?;

    // Check if user can borrow specified amount
    let mut uncollateralized_debt = false;
    if uncollateralized_loan_limit.is_zero() {
        // Collateralized loan: check max ltv is not exceeded
        let config = CONFIG.load(deps.storage)?;
        let oracle_address = address_provider::helpers::query_address(
            &deps.querier,
            config.address_provider_address,
            MarsContract::Oracle,
        )?;

        let user_position = get_user_position(
            deps.as_ref(),
            env.block.time.seconds(),
            &borrower_address,
            oracle_address.clone(),
            &user,
            global_state.market_count,
        )?;

        let borrow_asset_price = if is_borrowing_asset {
            // if user was already borrowing, get price from user position
            user_position.get_asset_price(asset_reference.as_slice(), &asset_label)?
        } else {
            mars::oracle::helpers::query_price(
                deps.querier,
                oracle_address,
                &asset_label,
                asset_reference.clone(),
                asset_type,
            )?
        };

        let borrow_amount_in_uusd = borrow_amount * borrow_asset_price;

        if user_position.total_debt_in_uusd + borrow_amount_in_uusd > user_position.max_debt_in_uusd
        {
            return Err(StdError::generic_err(
                "borrow amount exceeds maximum allowed given current collateral value",
            )
            .into());
        }
    } else {
        // Uncollateralized loan: check borrow amount plus debt does not exceed uncollateralized loan limit
        uncollateralized_debt = true;

        let borrower_debt = DEBTS
            .may_load(
                deps.storage,
                (asset_reference.as_slice(), &borrower_address),
            )?
            .unwrap_or(Debt {
                amount_scaled: Uint128::zero(),
                uncollateralized: uncollateralized_debt,
            });

        let asset_market = MARKETS.load(deps.storage, asset_reference.as_slice())?;
        let debt_amount = get_descaled_amount(
            borrower_debt.amount_scaled,
            get_updated_borrow_index(&asset_market, env.block.time.seconds()),
        );
        if borrow_amount + debt_amount > uncollateralized_loan_limit {
            return Err(StdError::generic_err(
                "borrow amount exceeds uncollateralized loan limit given existing debt",
            )
            .into());
        }
    }

    apply_accumulated_interests(&env, &mut borrow_market);

    let mut events = vec![];
    // Set borrowing asset for user
    if !is_borrowing_asset {
        set_bit(&mut user.borrowed_assets, borrow_market.index)?;
        USERS.save(deps.storage, &borrower_address, &user)?;
        events.push(build_debt_position_changed_event(
            asset_label.as_str(),
            true,
            borrower_address.to_string(),
        ));
    }

    // Set new debt
    let mut debt = DEBTS
        .may_load(
            deps.storage,
            (asset_reference.as_slice(), &borrower_address),
        )?
        .unwrap_or(Debt {
            amount_scaled: Uint128::zero(),
            uncollateralized: uncollateralized_debt,
        });
    let borrow_amount_scaled = get_scaled_amount(
        borrow_amount,
        get_updated_borrow_index(&borrow_market, env.block.time.seconds()),
    );
    debt.amount_scaled += borrow_amount_scaled;
    DEBTS.save(
        deps.storage,
        (asset_reference.as_slice(), &borrower_address),
        &debt,
    )?;

    borrow_market.debt_total_scaled += borrow_amount_scaled;

    update_interest_rates(
        &deps,
        &env,
        asset_reference.as_slice(),
        &mut borrow_market,
        borrow_amount,
    )?;
    MARKETS.save(deps.storage, asset_reference.as_slice(), &borrow_market)?;

    // Send borrow amount to borrower
    let send_msg = build_send_asset_msg(
        deps.as_ref(),
        env.contract.address,
        borrower_address.clone(),
        asset,
        borrow_amount,
    )?;

    events.push(build_interests_updated_event(
        asset_label.as_str(),
        &borrow_market,
    ));

    let res = Response::new()
        .add_attribute("action", "borrow")
        .add_attribute("market", asset_label.as_str())
        .add_attribute("user", borrower_address.as_str())
        .add_attribute("amount", borrow_amount)
        .add_events(events)
        .add_message(send_msg);
    Ok(res)
}

/// Handle the repay of native tokens. Refund extra funds if they exist
pub fn execute_repay(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    repayer_address: Addr,
    asset_reference: &[u8],
    asset_label: &str,
    repay_amount: Uint128,
    asset_type: AssetType,
) -> Result<Response, ContractError> {
    let mut market = MARKETS.load(deps.storage, asset_reference)?;

    // Get repay amount
    // Cannot repay zero amount
    if repay_amount.is_zero() {
        return Err(StdError::generic_err(format!(
            "Repay amount must be greater than 0 {}",
            asset_label,
        ))
        .into());
    }

    // Check new debt
    let mut debt = DEBTS.load(deps.storage, (asset_reference, &repayer_address))?;

    if debt.amount_scaled.is_zero() {
        return Err(StdError::generic_err("Cannot repay 0 debt").into());
    }

    apply_accumulated_interests(&env, &mut market);

    let mut repay_amount_scaled = get_scaled_amount(
        repay_amount,
        get_updated_borrow_index(&market, env.block.time.seconds()),
    );

    let mut messages = vec![];
    let mut refund_amount = Uint128::zero();
    if repay_amount_scaled > debt.amount_scaled {
        // refund any excess amounts
        refund_amount = get_descaled_amount(
            repay_amount_scaled - debt.amount_scaled,
            get_updated_borrow_index(&market, env.block.time.seconds()),
        );
        let refund_msg = match asset_type {
            AssetType::Native => build_send_native_asset_msg(
                deps.as_ref(),
                env.contract.address.clone(),
                repayer_address.clone(),
                asset_label,
                refund_amount,
            )?,
            AssetType::Cw20 => {
                let token_contract_addr = deps.api.addr_validate(asset_label)?;
                build_send_cw20_token_msg(
                    repayer_address.clone(),
                    token_contract_addr,
                    refund_amount,
                )?
            }
        };
        messages.push(refund_msg);
        repay_amount_scaled = debt.amount_scaled;
    }

    debt.amount_scaled -= repay_amount_scaled;
    DEBTS.save(deps.storage, (asset_reference, &repayer_address), &debt)?;

    if repay_amount_scaled > market.debt_total_scaled {
        return Err(StdError::generic_err("Amount to repay is greater than total debt").into());
    }
    market.debt_total_scaled -= repay_amount_scaled;
    update_interest_rates(&deps, &env, asset_reference, &mut market, Uint128::zero())?;
    MARKETS.save(deps.storage, asset_reference, &market)?;

    let mut events = vec![];
    if debt.amount_scaled.is_zero() {
        // Remove asset from borrowed assets
        let mut user = USERS.load(deps.storage, &repayer_address)?;
        unset_bit(&mut user.borrowed_assets, market.index)?;
        USERS.save(deps.storage, &repayer_address, &user)?;
        events.push(build_debt_position_changed_event(
            asset_label,
            false,
            repayer_address.to_string(),
        ));
    }

    events.push(build_interests_updated_event(asset_label, &market));

    let res = Response::new()
        .add_attribute("action", "repay")
        .add_attribute("market", asset_label)
        .add_attribute("user", repayer_address)
        .add_attribute("amount", repay_amount - refund_amount)
        .add_messages(messages)
        .add_events(events);
    Ok(res)
}

/// Execute loan liquidations on under-collateralized loans
pub fn execute_liquidate(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    liquidator_address: Addr,
    collateral_asset: Asset,
    debt_asset: Asset,
    user_address: Addr,
    sent_debt_asset_amount: Uint128,
    receive_ma_token: bool,
) -> Result<Response, ContractError> {
    let block_time = env.block.time.seconds();
    let (debt_asset_label, debt_asset_reference, _) = debt_asset.get_attributes();

    // 1. Validate liquidation
    // If user (contract) has a positive uncollateralized limit then the user
    // cannot be liquidated
    if let Some(limit) = UNCOLLATERALIZED_LOAN_LIMITS.may_load(
        deps.storage,
        (debt_asset_reference.as_slice(), &user_address),
    )? {
        if !limit.is_zero() {
            return Err(StdError::generic_err(
                "user has a positive uncollateralized loan limit and thus cannot be liquidated",
            )
            .into());
        }
    };

    // liquidator must send positive amount of funds in the debt asset
    if sent_debt_asset_amount.is_zero() {
        return Err(StdError::generic_err(format!(
            "Must send more than 0 {} in order to liquidate",
            debt_asset_label,
        ))
        .into());
    }

    let (collateral_asset_label, collateral_asset_reference, _) = collateral_asset.get_attributes();

    let mut collateral_market =
        MARKETS.load(deps.storage, collateral_asset_reference.as_slice())?;

    // check if user has available collateral in specified collateral asset to be liquidated
    let user_collateral_balance_scaled = cw20_get_balance(
        &deps.querier,
        collateral_market.ma_token_address.clone(),
        user_address.clone(),
    )?;
    let user_collateral_balance = get_descaled_amount(
        user_collateral_balance_scaled,
        get_updated_liquidity_index(&collateral_market, block_time),
    );
    if user_collateral_balance.is_zero() {
        return Err(StdError::generic_err(
            "user has no balance in specified collateral asset to be liquidated",
        )
        .into());
    }

    // check if user has outstanding debt in the deposited asset that needs to be repayed
    let user_debt = DEBTS.load(
        deps.storage,
        (debt_asset_reference.as_slice(), &user_address),
    )?;
    if user_debt.amount_scaled.is_zero() {
        return Err(StdError::generic_err("User has no outstanding debt in the specified debt asset and thus cannot be liquidated").into());
    }

    // 2. Compute health factor
    let config = CONFIG.load(deps.storage)?;
    let global_state = GLOBAL_STATE.load(deps.storage)?;
    let user = USERS.load(deps.storage, &user_address)?;
    let oracle_address = address_provider::helpers::query_address(
        &deps.querier,
        config.address_provider_address,
        MarsContract::Oracle,
    )?;
    let user_position = get_user_position(
        deps.as_ref(),
        block_time,
        &user_address,
        oracle_address,
        &user,
        global_state.market_count,
    )?;

    let health_factor = match user_position.health_status {
        // NOTE: Should not get in practice as it would fail on the debt asset check
        UserHealthStatus::NotBorrowing => {
            return Err(StdError::generic_err(
                "User has no outstanding debt and thus cannot be liquidated",
            )
            .into())
        }
        UserHealthStatus::Borrowing(hf) => hf,
    };

    // if health factor is not less than one the user cannot be liquidated
    if health_factor >= Decimal::one() {
        return Err(StdError::generic_err(
            "User's health factor is not less than 1 and thus cannot be liquidated",
        )
        .into());
    }

    let mut debt_market = MARKETS.load(deps.storage, debt_asset_reference.as_slice())?;

    // 3. Compute debt to repay and collateral to liquidate
    let collateral_price = user_position.get_asset_price(
        collateral_asset_reference.as_slice(),
        &collateral_asset_label,
    )?;
    let debt_price =
        user_position.get_asset_price(debt_asset_reference.as_slice(), &debt_asset_label)?;

    apply_accumulated_interests(&env, &mut debt_market);

    let user_debt_asset_total_debt = get_descaled_amount(
        user_debt.amount_scaled,
        get_updated_borrow_index(&debt_market, block_time),
    );

    let (debt_amount_to_repay, collateral_amount_to_liquidate, refund_amount) =
        liquidation_compute_amounts(
            collateral_price,
            debt_price,
            config.close_factor,
            user_collateral_balance,
            collateral_market.liquidation_bonus,
            user_debt_asset_total_debt,
            sent_debt_asset_amount,
        );

    let mut messages = vec![];
    let mut events = vec![];

    // 4. Update collateral positions and market depending on whether the liquidator elects to
    // receive ma_tokens or the underlying asset
    if receive_ma_token {
        process_ma_token_transfer_to_liquidator(
            deps.branch(),
            block_time,
            &user_address,
            &liquidator_address,
            collateral_asset_label.as_str(),
            &collateral_market,
            collateral_amount_to_liquidate,
            &mut messages,
            &mut events,
        )?;
    } else {
        process_underlying_asset_transfer_to_liquidator(
            deps.branch(),
            &env,
            &user_address,
            &liquidator_address,
            collateral_asset,
            collateral_asset_reference.as_slice(),
            &mut collateral_market,
            collateral_amount_to_liquidate,
            &mut messages,
        )?;
    }

    // if max collateral to liquidate equals the user's balance then unset collateral bit
    if collateral_amount_to_liquidate == user_collateral_balance {
        let mut user = USERS.load(deps.storage, &user_address)?;
        unset_bit(&mut user.collateral_assets, collateral_market.index)?;
        USERS.save(deps.storage, &user_address, &user)?;
        events.push(build_collateral_position_changed_event(
            collateral_asset_label.as_str(),
            false,
            user_address.to_string(),
        ));
    }

    // 5. Update debt market and positions

    let debt_amount_to_repay_scaled = get_scaled_amount(
        debt_amount_to_repay,
        get_updated_borrow_index(&debt_market, block_time),
    );

    // update user and market debt
    let mut debt = DEBTS.load(
        deps.storage,
        (debt_asset_reference.as_slice(), &user_address),
    )?;
    // NOTE: Should be > 0 as amount to repay is capped by the close factor
    debt.amount_scaled -= debt_amount_to_repay_scaled;
    DEBTS.save(
        deps.storage,
        (debt_asset_reference.as_slice(), &user_address),
        &debt,
    )?;
    debt_market.debt_total_scaled -= debt_amount_to_repay_scaled;

    update_interest_rates(
        &deps,
        &env,
        debt_asset_reference.as_slice(),
        &mut debt_market,
        refund_amount,
    )?;

    // save markets
    MARKETS.save(deps.storage, debt_asset_reference.as_slice(), &debt_market)?;
    MARKETS.save(
        deps.storage,
        collateral_asset_reference.as_slice(),
        &collateral_market,
    )?;

    // 6. Build response
    // refund sent amount in excess of actual debt amount to liquidate
    if refund_amount > Uint128::zero() {
        let refund_msg = build_send_asset_msg(
            deps.as_ref(),
            env.contract.address,
            liquidator_address.clone(),
            debt_asset,
            refund_amount,
        )?;
        messages.push(refund_msg);
    }

    events.push(build_interests_updated_event(
        debt_asset_label.as_str(),
        &debt_market,
    ));
    if !receive_ma_token {
        events.push(build_interests_updated_event(
            collateral_asset_label.as_str(),
            &collateral_market,
        ));
    }

    let res = Response::new()
        .add_attribute("action", "liquidate")
        .add_attribute("collateral_market", collateral_asset_label.as_str())
        .add_attribute("debt_market", debt_asset_label.as_str())
        .add_attribute("user", user_address.as_str())
        .add_attribute("liquidator", liquidator_address.as_str())
        .add_attribute(
            "collateral_amount_liquidated",
            collateral_amount_to_liquidate.to_string(),
        )
        .add_attribute("debt_amount_repaid", debt_amount_to_repay.to_string())
        .add_attribute("refund_amount", refund_amount.to_string())
        .add_events(events)
        .add_messages(messages);
    Ok(res)
}

/// Transfer ma tokens from user to liquidator
fn process_ma_token_transfer_to_liquidator(
    deps: DepsMut,
    block_time: u64,
    user_addr: &Addr,
    liquidator_addr: &Addr,
    collateral_asset_label: &str,
    collateral_market: &Market,
    collateral_amount_to_liquidate: Uint128,
    messages: &mut Vec<CosmosMsg>,
    events: &mut Vec<Event>,
) -> StdResult<()> {
    let mut liquidator = USERS
        .may_load(deps.storage, liquidator_addr)?
        .unwrap_or_default();

    // Set liquidator's deposited bit to true if not already true
    // NOTE: previous checks should ensure this amount is not zero
    let liquidator_is_using_as_collateral =
        get_bit(liquidator.collateral_assets, collateral_market.index)?;
    if !liquidator_is_using_as_collateral {
        set_bit(&mut liquidator.collateral_assets, collateral_market.index)?;
        USERS.save(deps.storage, liquidator_addr, &liquidator)?;
        events.push(build_collateral_position_changed_event(
            collateral_asset_label,
            true,
            liquidator_addr.to_string(),
        ));
    }

    let collateral_amount_to_liquidate_scaled = get_scaled_amount(
        collateral_amount_to_liquidate,
        get_updated_liquidity_index(collateral_market, block_time),
    );

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: collateral_market.ma_token_address.to_string(),
        msg: to_binary(&mars::ma_token::msg::ExecuteMsg::TransferOnLiquidation {
            sender: user_addr.to_string(),
            recipient: liquidator_addr.to_string(),
            amount: collateral_amount_to_liquidate_scaled,
        })?,
        funds: vec![],
    }));

    Ok(())
}

/// Burn ma_tokens from user and send underlying asset to liquidator
fn process_underlying_asset_transfer_to_liquidator(
    deps: DepsMut,
    env: &Env,
    user_addr: &Addr,
    liquidator_addr: &Addr,
    collateral_asset: Asset,
    collateral_asset_reference: &[u8],
    mut collateral_market: &mut Market,
    collateral_amount_to_liquidate: Uint128,
    messages: &mut Vec<CosmosMsg>,
) -> StdResult<()> {
    let block_time = env.block.time.seconds();

    // Ensure contract has enough collateral to send back underlying asset
    let contract_collateral_balance = match collateral_asset.clone() {
        Asset::Native { denom } => {
            deps.querier
                .query_balance(env.contract.address.clone(), denom.as_str())?
                .amount
        }
        Asset::Cw20 {
            contract_addr: token_addr,
        } => {
            let token_addr = deps.api.addr_validate(&token_addr)?;
            cw20_get_balance(&deps.querier, token_addr, env.contract.address.clone())?
        }
    };

    if contract_collateral_balance < collateral_amount_to_liquidate {
        return Err(StdError::generic_err(
            "contract does not have enough collateral liquidity to send back underlying asset",
        ));
    }

    // Apply update collateral interest as liquidity is reduced
    apply_accumulated_interests(env, &mut collateral_market);
    update_interest_rates(
        &deps,
        env,
        collateral_asset_reference,
        &mut collateral_market,
        collateral_amount_to_liquidate,
    )?;

    let collateral_amount_to_liquidate_scaled = get_scaled_amount(
        collateral_amount_to_liquidate,
        get_updated_liquidity_index(collateral_market, block_time),
    );

    let burn_ma_tokens_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: collateral_market.ma_token_address.to_string(),
        msg: to_binary(&mars::ma_token::msg::ExecuteMsg::Burn {
            user: user_addr.to_string(),

            amount: collateral_amount_to_liquidate_scaled,
        })?,
        funds: vec![],
    });

    let send_underlying_asset_msg = build_send_asset_msg(
        deps.as_ref(),
        env.contract.address.clone(),
        liquidator_addr.clone(),
        collateral_asset,
        collateral_amount_to_liquidate,
    )?;
    messages.push(burn_ma_tokens_msg);
    messages.push(send_underlying_asset_msg);

    Ok(())
}

/// Computes debt to repay (in debt asset),
/// collateral to liquidate (in collateral asset) and
/// amount to refund the liquidator (in debt asset)
fn liquidation_compute_amounts(
    collateral_price: Decimal,
    debt_price: Decimal,
    close_factor: Decimal,
    user_collateral_balance: Uint128,
    liquidation_bonus: Decimal,
    user_debt_asset_total_debt: Uint128,
    sent_debt_asset_amount: Uint128,
) -> (Uint128, Uint128, Uint128) {
    // Debt: Only up to a fraction of the total debt (determined by the close factor) can be
    // repayed.
    let max_repayable_debt = close_factor * user_debt_asset_total_debt;

    let mut debt_amount_to_repay = if sent_debt_asset_amount > max_repayable_debt {
        max_repayable_debt
    } else {
        sent_debt_asset_amount
    };

    // Collateral: debt to repay in uusd times the liquidation
    // bonus
    let debt_amount_to_repay_in_uusd = debt_amount_to_repay * debt_price;
    let collateral_amount_to_liquidate_in_uusd =
        debt_amount_to_repay_in_uusd * (Decimal::one() + liquidation_bonus);
    let mut collateral_amount_to_liquidate =
        collateral_amount_to_liquidate_in_uusd * reverse_decimal(collateral_price);

    // If collateral amount to liquidate is higher than user_collateral_balance,
    // liquidate the full balance and adjust the debt amount to repay accordingly
    if collateral_amount_to_liquidate > user_collateral_balance {
        collateral_amount_to_liquidate = user_collateral_balance;
        debt_amount_to_repay = (collateral_price * collateral_amount_to_liquidate)
            * reverse_decimal(debt_price)
            * reverse_decimal(Decimal::one() + liquidation_bonus);
    }

    let refund_amount = sent_debt_asset_amount - debt_amount_to_repay;

    (
        debt_amount_to_repay,
        collateral_amount_to_liquidate,
        refund_amount,
    )
}

/// Update uncollateralized loan limit by a given amount in uusd
pub fn execute_finalize_liquidity_token_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from_address: Addr,
    to_address: Addr,
    from_previous_balance: Uint128,
    to_previous_balance: Uint128,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // Get liquidity token market
    let market_reference = MARKET_REFERENCES_BY_MA_TOKEN.load(deps.storage, &info.sender)?;
    let market = MARKETS.load(deps.storage, market_reference.as_slice())?;

    // Check user health factor is above 1
    let global_state = GLOBAL_STATE.load(deps.storage)?;
    let mut from_user = USERS.load(deps.storage, &from_address)?;
    let config = CONFIG.load(deps.storage)?;
    let oracle_address = address_provider::helpers::query_address(
        &deps.querier,
        config.address_provider_address,
        MarsContract::Oracle,
    )?;
    let user_position = get_user_position(
        deps.as_ref(),
        env.block.time.seconds(),
        &from_address,
        oracle_address,
        &from_user,
        global_state.market_count,
    )?;
    if let UserHealthStatus::Borrowing(health_factor) = user_position.health_status {
        if health_factor < Decimal::one() {
            return Err(StdError::generic_err("Cannot make token transfer if it results in a health factor lower than 1 for the sender").into());
        }
    }

    let asset_label = String::from_utf8(market_reference).expect("Found invalid UTF-8");
    let mut events = vec![];

    // Update users's positions
    if from_address != to_address {
        if from_previous_balance.checked_sub(amount)?.is_zero() {
            unset_bit(&mut from_user.collateral_assets, market.index)?;
            USERS.save(deps.storage, &from_address, &from_user)?;
            events.push(build_collateral_position_changed_event(
                asset_label.as_str(),
                false,
                from_address.to_string(),
            ))
        }

        if to_previous_balance.is_zero() && !amount.is_zero() {
            let mut to_user = USERS
                .may_load(deps.storage, &to_address)?
                .unwrap_or_default();
            set_bit(&mut to_user.collateral_assets, market.index)?;
            USERS.save(deps.storage, &to_address, &to_user)?;
            events.push(build_collateral_position_changed_event(
                asset_label.as_str(),
                true,
                to_address.to_string(),
            ))
        }
    }

    let res = Response::new()
        .add_attribute("action", "finalize_liquidity_token_transfer")
        .add_events(events);
    Ok(res)
}

/// Update uncollateralized loan limit by a given amount in uusd
pub fn execute_update_uncollateralized_loan_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    user_address: Addr,
    asset: Asset,
    new_limit: Uint128,
) -> Result<Response, ContractError> {
    // Get config
    let config = CONFIG.load(deps.storage)?;

    // Only owner can do this
    if info.sender != config.owner {
        return Err(MarsError::Unauthorized {}.into());
    }

    let (asset_label, asset_reference, _) = asset.get_attributes();

    UNCOLLATERALIZED_LOAN_LIMITS.save(
        deps.storage,
        (asset_reference.as_slice(), &user_address),
        &new_limit,
    )?;

    DEBTS.update(
        deps.storage,
        (asset_reference.as_slice(), &user_address),
        |debt_opt: Option<Debt>| -> StdResult<_> {
            let mut debt = debt_opt.unwrap_or(Debt {
                amount_scaled: Uint128::zero(),
                uncollateralized: false,
            });
            // if limit == 0 then uncollateralized = false, otherwise uncollateralized = true
            debt.uncollateralized = !new_limit.is_zero();
            Ok(debt)
        },
    )?;

    let res = Response::new()
        .add_attribute("action", "update_uncollateralized_loan_limit")
        .add_attribute("user", user_address.as_str())
        .add_attribute("asset", asset_label)
        .add_attribute("new_allowance", new_limit.to_string());
    Ok(res)
}

/// Update (enable / disable) collateral asset for specific user
pub fn execute_update_user_collateral_asset_status(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset: Asset,
    enable: bool,
) -> Result<Response, ContractError> {
    let user_address = info.sender;
    let mut user = USERS
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let mut events = vec![];

    let (collateral_asset_label, collateral_asset_reference, _) = asset.get_attributes();
    let collateral_market = MARKETS.load(deps.storage, collateral_asset_reference.as_slice())?;
    let has_collateral_asset = get_bit(user.collateral_assets, collateral_market.index)?;
    if !has_collateral_asset && enable {
        let collateral_ma_address = collateral_market.ma_token_address;
        let user_collateral_balance =
            cw20_get_balance(&deps.querier, collateral_ma_address, user_address.clone())?;
        if user_collateral_balance > Uint128::zero() {
            // enable collateral asset
            set_bit(&mut user.collateral_assets, collateral_market.index)?;
            USERS.save(deps.storage, &user_address, &user)?;
            events.push(build_collateral_position_changed_event(
                collateral_asset_label.as_str(),
                true,
                user_address.to_string(),
            ));
        } else {
            return Err(StdError::generic_err(format!(
                "User address {} has no balance in specified collateral asset {}",
                user_address.as_str(),
                collateral_asset_label
            ))
            .into());
        }
    } else if has_collateral_asset && !enable {
        // disable collateral asset
        unset_bit(&mut user.collateral_assets, collateral_market.index)?;
        USERS.save(deps.storage, &user_address, &user)?;
        events.push(build_collateral_position_changed_event(
            collateral_asset_label.as_str(),
            false,
            user_address.to_string(),
        ));
    }

    let res = Response::new()
        .add_attribute("action", "update_user_collateral_asset_status")
        .add_attribute("user", user_address.as_str())
        .add_attribute("asset", collateral_asset_label)
        .add_attribute("has_collateral", has_collateral_asset.to_string())
        .add_attribute("enable", enable.to_string())
        .add_events(events);
    Ok(res)
}

/// Send accumulated asset income to protocol contracts
pub fn execute_distribute_protocol_income(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    asset: Asset,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    // Get config
    let config = CONFIG.load(deps.storage)?;

    let (asset_label, asset_reference, _) = asset.get_attributes();
    let mut market = MARKETS.load(deps.storage, asset_reference.as_slice())?;

    let amount_to_distribute = match amount {
        Some(amount) => amount,
        None => market.protocol_income_to_distribute,
    };

    if amount_to_distribute > market.protocol_income_to_distribute {
        return Err(StdError::generic_err(
            "amount specified exceeds market's income to be distributed",
        )
        .into());
    }

    market.protocol_income_to_distribute -= amount_to_distribute;
    MARKETS.save(deps.storage, asset_reference.as_slice(), &market)?;

    let mars_contracts = vec![
        MarsContract::InsuranceFund,
        MarsContract::Staking,
        MarsContract::Treasury,
    ];
    let mut addresses_query = address_provider::helpers::query_addresses(
        &deps.querier,
        config.address_provider_address,
        mars_contracts,
    )?;

    let treasury_address = addresses_query.pop().unwrap();
    let staking_address = addresses_query.pop().unwrap();
    let insurance_fund_address = addresses_query.pop().unwrap();

    let insurance_fund_amount = amount_to_distribute * config.insurance_fund_fee_share;
    let treasury_amount = amount_to_distribute * config.treasury_fee_share;
    let amount_to_distribute_before_staking_rewards = insurance_fund_amount + treasury_amount;
    if amount_to_distribute_before_staking_rewards > amount_to_distribute {
        return Err(StdError::generic_err(format!(
            "Decimal256 Underflow: will subtract {} from {} ",
            amount_to_distribute_before_staking_rewards, amount_to_distribute
        ))
        .into());
    }
    let staking_amount = amount_to_distribute - amount_to_distribute_before_staking_rewards;

    let mut messages = vec![];
    // only build and add send message if fee is non-zero
    if !insurance_fund_amount.is_zero() {
        let insurance_fund_msg = build_send_asset_msg(
            deps.as_ref(),
            env.contract.address.clone(),
            insurance_fund_address,
            asset.clone(),
            insurance_fund_amount,
        )?;
        messages.push(insurance_fund_msg);
    }

    if !treasury_amount.is_zero() {
        let scaled_mint_amount = get_scaled_amount(
            treasury_amount,
            get_updated_liquidity_index(&market, env.block.time.seconds()),
        );
        let treasury_fund_msg = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: market.ma_token_address.into(),
            msg: to_binary(&Cw20ExecuteMsg::Mint {
                recipient: treasury_address.into(),
                amount: scaled_mint_amount,
            })?,
            funds: vec![],
        });
        messages.push(treasury_fund_msg);
    }

    if !staking_amount.is_zero() {
        let staking_msg = build_send_asset_msg(
            deps.as_ref(),
            env.contract.address,
            staking_address,
            asset,
            staking_amount,
        )?;
        messages.push(staking_msg);
    }

    let res = Response::new()
        .add_attribute("action", "distribute_protocol_income")
        .add_attribute("asset", asset_label)
        .add_attribute("amount", amount_to_distribute)
        .add_messages(messages);
    Ok(res)
}

// QUERIES

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Market { asset } => to_binary(&query_market(deps, asset)?),
        QueryMsg::MarketsList {} => to_binary(&query_markets_list(deps)?),
        QueryMsg::UserDebt { user_address } => {
            let address = deps.api.addr_validate(&user_address)?;
            to_binary(&query_debt(deps, address)?)
        }
        QueryMsg::UserCollateral { user_address } => {
            let address = deps.api.addr_validate(&user_address)?;
            to_binary(&query_collateral(deps, address)?)
        }
        QueryMsg::UncollateralizedLoanLimit {
            user_address,
            asset,
        } => {
            let user_address = deps.api.addr_validate(&user_address)?;
            to_binary(&query_uncollateralized_loan_limit(
                deps,
                user_address,
                asset,
            )?)
        }
        QueryMsg::ScaledLiquidityAmount { asset, amount } => {
            to_binary(&query_scaled_liquidity_amount(deps, env, asset, amount)?)
        }
        QueryMsg::ScaledDebtAmount { asset, amount } => {
            to_binary(&query_scaled_debt_amount(deps, env, asset, amount)?)
        }
        QueryMsg::DescaledLiquidityAmount {
            ma_token_address,
            amount,
        } => to_binary(&query_descaled_liquidity_amount(
            deps,
            env,
            ma_token_address,
            amount,
        )?),
        QueryMsg::UserPosition { user_address } => {
            let address = deps.api.addr_validate(&user_address)?;
            to_binary(&query_user_position(deps, env, address)?)
        }
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let money_market = GLOBAL_STATE.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner,
        address_provider_address: config.address_provider_address,
        insurance_fund_fee_share: config.insurance_fund_fee_share,
        treasury_fee_share: config.treasury_fee_share,
        ma_token_code_id: config.ma_token_code_id,
        market_count: money_market.market_count,
        close_factor: config.close_factor,
    })
}

fn query_market(deps: Deps, asset: Asset) -> StdResult<MarketResponse> {
    let (label, reference, _) = asset.get_attributes();
    let market = match MARKETS.load(deps.storage, reference.as_slice()) {
        Ok(market) => market,
        Err(_) => {
            return Err(StdError::generic_err(format!(
                "failed to load market for: {}",
                label
            )))
        }
    };

    Ok(MarketResponse {
        ma_token_address: market.ma_token_address,
        borrow_index: market.borrow_index,
        liquidity_index: market.liquidity_index,
        borrow_rate: market.borrow_rate,
        liquidity_rate: market.liquidity_rate,
        max_loan_to_value: market.max_loan_to_value,
        interests_last_updated: market.interests_last_updated,
        debt_total_scaled: market.debt_total_scaled,
        asset_type: market.asset_type,
        maintenance_margin: market.maintenance_margin,
        liquidation_bonus: market.liquidation_bonus,
    })
}

fn query_markets_list(deps: Deps) -> StdResult<MarketsListResponse> {
    let markets_list: StdResult<Vec<_>> = MARKETS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            let denom = get_market_denom(deps, k, v.asset_type)?;

            Ok(MarketInfo {
                denom,
                ma_token_address: v.ma_token_address,
            })
        })
        .collect();

    Ok(MarketsListResponse {
        markets_list: markets_list?,
    })
}

fn query_debt(deps: Deps, address: Addr) -> StdResult<DebtResponse> {
    let user = USERS.may_load(deps.storage, &address)?.unwrap_or_default();

    let debts: StdResult<Vec<_>> = MARKETS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            let denom = get_market_denom(deps, k.clone(), v.asset_type)?;

            let is_borrowing_asset = get_bit(user.borrowed_assets, v.index)?;
            if is_borrowing_asset {
                let debt = DEBTS.load(deps.storage, (k.as_slice(), &address))?;
                Ok(DebtInfo {
                    denom,
                    amount_scaled: debt.amount_scaled,
                })
            } else {
                Ok(DebtInfo {
                    denom,
                    amount_scaled: Uint128::zero(),
                })
            }
        })
        .collect();

    Ok(DebtResponse { debts: debts? })
}

fn query_collateral(deps: Deps, address: Addr) -> StdResult<CollateralResponse> {
    let user = USERS.may_load(deps.storage, &address)?.unwrap_or_default();

    let collateral: StdResult<Vec<_>> = MARKETS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (k, v) = item?;
            let denom = get_market_denom(deps, k, v.asset_type)?;

            Ok(CollateralInfo {
                denom,
                enabled: get_bit(user.collateral_assets, v.index)?,
            })
        })
        .collect();

    Ok(CollateralResponse {
        collateral: collateral?,
    })
}

fn query_uncollateralized_loan_limit(
    deps: Deps,
    user_address: Addr,
    asset: Asset,
) -> StdResult<UncollateralizedLoanLimitResponse> {
    let (asset_label, asset_reference, _) = asset.get_attributes();
    let uncollateralized_loan_limit = UNCOLLATERALIZED_LOAN_LIMITS
        .load(deps.storage, (asset_reference.as_slice(), &user_address));

    match uncollateralized_loan_limit {
        Ok(limit) => Ok(UncollateralizedLoanLimitResponse { limit }),
        Err(_) => Err(StdError::not_found(format!(
            "No uncollateralized loan approved for user_address: {} on asset: {}",
            user_address, asset_label
        ))),
    }
}

fn query_scaled_liquidity_amount(
    deps: Deps,
    env: Env,
    asset: Asset,
    amount: Uint128,
) -> StdResult<AmountResponse> {
    let asset_reference = asset.get_reference();
    let market = MARKETS.load(deps.storage, asset_reference.as_slice())?;
    let scaled_amount = get_scaled_amount(
        amount,
        get_updated_liquidity_index(&market, env.block.time.seconds()),
    );
    Ok(AmountResponse {
        amount: scaled_amount,
    })
}

fn query_scaled_debt_amount(
    deps: Deps,
    env: Env,
    asset: Asset,
    amount: Uint128,
) -> StdResult<AmountResponse> {
    let asset_reference = asset.get_reference();
    let market = MARKETS.load(deps.storage, asset_reference.as_slice())?;
    let scaled_amount = get_scaled_amount(
        amount,
        get_updated_borrow_index(&market, env.block.time.seconds()),
    );
    Ok(AmountResponse {
        amount: scaled_amount,
    })
}

fn query_descaled_liquidity_amount(
    deps: Deps,
    env: Env,
    ma_token_address: String,
    amount: Uint128,
) -> StdResult<AmountResponse> {
    let ma_token_address = deps.api.addr_validate(&ma_token_address)?;
    let market_reference = MARKET_REFERENCES_BY_MA_TOKEN.load(deps.storage, &ma_token_address)?;
    let market = MARKETS.load(deps.storage, market_reference.as_slice())?;
    let descaled_amount = get_descaled_amount(
        amount,
        get_updated_liquidity_index(&market, env.block.time.seconds()),
    );
    Ok(AmountResponse {
        amount: descaled_amount,
    })
}

fn query_user_position(deps: Deps, env: Env, address: Addr) -> StdResult<UserPositionResponse> {
    let config = CONFIG.load(deps.storage)?;
    let global_state = GLOBAL_STATE.load(deps.storage)?;
    let user = USERS.load(deps.storage, &address)?;
    let oracle_address = address_provider::helpers::query_address(
        &deps.querier,
        config.address_provider_address,
        MarsContract::Oracle,
    )?;
    let user_position = get_user_position(
        deps,
        env.block.time.seconds(),
        &address,
        oracle_address,
        &user,
        global_state.market_count,
    )?;

    Ok(UserPositionResponse {
        total_collateral_in_uusd: user_position.total_collateral_in_uusd,
        total_debt_in_uusd: user_position.total_debt_in_uusd,
        total_collateralized_debt_in_uusd: user_position.total_collateralized_debt_in_uusd,
        max_debt_in_uusd: user_position.max_debt_in_uusd,
        weighted_maintenance_margin_in_uusd: user_position.weighted_maintenance_margin_in_uusd,
        health_status: user_position.health_status,
    })
}

// EVENTS

fn build_interests_updated_event(label: &str, market: &Market) -> Event {
    Event::new("interests_updated")
        .add_attribute("market", label)
        .add_attribute("borrow_index", market.borrow_index.to_string())
        .add_attribute("liquidity_index", market.liquidity_index.to_string())
        .add_attribute("borrow_rate", market.borrow_rate.to_string())
        .add_attribute("liquidity_rate", market.liquidity_rate.to_string())
}

fn build_collateral_position_changed_event(label: &str, enabled: bool, user_addr: String) -> Event {
    Event::new("collateral_position_changed")
        .add_attribute("market", label)
        .add_attribute("using_as_collateral", enabled.to_string())
        .add_attribute("user", user_addr)
}

fn build_debt_position_changed_event(label: &str, enabled: bool, user_addr: String) -> Event {
    Event::new("debt_position_changed")
        .add_attribute("market", label)
        .add_attribute("borrowing", enabled.to_string())
        .add_attribute("user", user_addr)
}

// HELPERS

// native coins
fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint128 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| c.amount)
        .unwrap_or_else(Uint128::zero)
}

fn get_market_denom(
    deps: Deps,
    market_reference: Vec<u8>,
    asset_type: AssetType,
) -> StdResult<String> {
    match asset_type {
        AssetType::Native => match String::from_utf8(market_reference) {
            Ok(denom) => Ok(denom),
            Err(_) => Err(StdError::generic_err("failed to encode key into string")),
        },
        AssetType::Cw20 => {
            let cw20_contract_address = match String::from_utf8(market_reference) {
                Ok(cw20_contract_address) => cw20_contract_address,
                Err(_) => {
                    return Err(StdError::generic_err(
                        "failed to encode key into contract address",
                    ))
                }
            };
            let cw20_contract_address = deps.api.addr_validate(&cw20_contract_address)?;
            match cw20_get_symbol(&deps.querier, cw20_contract_address.clone()) {
                Ok(symbol) => Ok(symbol),
                Err(_) => {
                    return Err(StdError::generic_err(format!(
                        "failed to get symbol from cw20 contract address: {}",
                        cw20_contract_address
                    )));
                }
            }
        }
    }
}

// bitwise operations
/// Gets bit: true: 1, false: 0
pub fn get_bit(bitmap: Uint128, index: u32) -> StdResult<bool> {
    if index >= 128 {
        return Err(StdError::generic_err("index out of range"));
    }
    Ok(((bitmap.u128() >> index) & 1) == 1)
}

/// Sets bit to 1
fn set_bit(bitmap: &mut Uint128, index: u32) -> StdResult<()> {
    if index >= 128 {
        return Err(StdError::generic_err("index out of range"));
    }
    *bitmap = Uint128::from(bitmap.u128() | (1 << index));
    Ok(())
}

/// Sets bit to 0
fn unset_bit(bitmap: &mut Uint128, index: u32) -> StdResult<()> {
    if index >= 128 {
        return Err(StdError::generic_err("index out of range"));
    }
    *bitmap = Uint128::from(bitmap.u128() & !(1 << index));
    Ok(())
}

fn build_send_asset_msg(
    deps: Deps,
    sender_address: Addr,
    recipient_address: Addr,
    asset: Asset,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    match asset {
        Asset::Native { denom } => Ok(build_send_native_asset_msg(
            deps,
            sender_address,
            recipient_address,
            denom.as_str(),
            amount,
        )?),
        Asset::Cw20 { contract_addr } => {
            let contract_addr = deps.api.addr_validate(&contract_addr)?;
            build_send_cw20_token_msg(recipient_address, contract_addr, amount)
        }
    }
}

/// Prepare BankMsg::Send message.
/// When doing native transfers a "tax" is charged.
/// The actual amount taken from the contract is: amount + tax.
/// Instead of sending amount, send: amount - compute_tax(amount).
fn build_send_native_asset_msg(
    deps: Deps,
    _sender: Addr,
    recipient: Addr,
    denom: &str,
    amount: Uint128,
) -> StdResult<CosmosMsg> {
    Ok(CosmosMsg::Bank(BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![deduct_tax(
            deps,
            Coin {
                denom: denom.to_string(),
                amount,
            },
        )?],
    }))
}

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

pub fn market_get_from_index(deps: &Deps, index: u32) -> StdResult<(Vec<u8>, Market)> {
    let asset_reference_vec =
        match MARKET_REFERENCES_BY_INDEX.load(deps.storage, U32Key::new(index)) {
            Ok(asset_reference_vec) => asset_reference_vec,
            Err(_) => {
                return Err(StdError::generic_err(format!(
                    "no market reference exists with index: {}",
                    index
                )))
            }
        };

    match MARKETS.load(deps.storage, asset_reference_vec.as_slice()) {
        Ok(asset_market) => Ok((asset_reference_vec, asset_market)),
        Err(_) => Err(StdError::generic_err(format!(
            "no asset market exists with asset reference: {}",
            String::from_utf8(asset_reference_vec).expect("Found invalid UTF-8")
        ))),
    }
}
