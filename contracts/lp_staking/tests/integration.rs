use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128, Uint64};
use cw20_base::msg::ExecuteMsg as CW20ExecuteMsg;
use cw_multi_test::{App, BankKeeper, ContractWrapper, Executor};
use mars_periphery::lp_staking::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
    StakerInfoResponse, StateResponse, TimeResponse, UpdateConfigMsg,
};

fn mock_app() -> App {
    let api = MockApi::default();
    let env = mock_env();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();

    App::new(api, env.block, bank, storage)
}

// Instantiate MARS Token Contract
fn instantiate_mars_token(app: &mut App, owner: Addr) -> Addr {
    let mars_token_contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    let mars_token_code_id = app.store_code(mars_token_contract);

    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Astro token"),
        symbol: String::from("ASTRO"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let mars_token_instance = app
        .instantiate_contract(
            mars_token_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("ASTRO"),
            None,
        )
        .unwrap();
    mars_token_instance
}

// Instantiate LP STAKING Contract
fn instantiate_lp_staking_contract(
    app: &mut App,
    owner: Addr,
    mars_token_instance: Addr,
) -> (Addr, Addr, InstantiateMsg) {
    let staking_token_contract = Box::new(ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    let staking_token_code_id = app.store_code(staking_token_contract);

    let msg = cw20_base::msg::InstantiateMsg {
        name: String::from("Astro LP token"),
        symbol: String::from("aLP"),
        decimals: 6,
        initial_balances: vec![],
        mint: Some(cw20::MinterResponse {
            minter: owner.to_string(),
            cap: None,
        }),
        marketing: None,
    };

    let staking_token_instance = app
        .instantiate_contract(
            staking_token_code_id,
            owner.clone(),
            &msg,
            &[],
            String::from("aLP"),
            None,
        )
        .unwrap();

    let lp_staking_contract = Box::new(ContractWrapper::new(
        mars_lp_staking::contract::execute,
        mars_lp_staking::contract::instantiate,
        mars_lp_staking::contract::query,
    ));

    let lp_staking_code_id = app.store_code(lp_staking_contract);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_000)
    });
    let lp_staking_instantiate_msg = mars_periphery::lp_staking::InstantiateMsg {
        owner: Some(owner.clone().to_string()),
        mars_token: mars_token_instance.clone().to_string(),
        staking_token: Some(staking_token_instance.to_string()),
        init_timestamp: 1_000_000_001,
        till_timestamp: 1_000_000_00000,
        cycle_rewards: Some(Uint128::from(100u64)),
        cycle_duration: 10u64,
        reward_increase: Some(Decimal::from_ratio(2u64, 100u64)),
    };

    // Init contract
    let lp_staking_instance = app
        .instantiate_contract(
            lp_staking_code_id,
            owner.clone(),
            &lp_staking_instantiate_msg,
            &[],
            "lp_staking",
            None,
        )
        .unwrap();

    (
        lp_staking_instance,
        staking_token_instance,
        lp_staking_instantiate_msg,
    )
}

// Initiates Mars token and LP Staking token
fn init_all_contracts(app: &mut App, owner: Addr) -> (Addr, Addr, Addr, InstantiateMsg) {
    let mars_token_instance = instantiate_mars_token(app, owner.clone());

    let (lp_staking_instance, staking_token_instance, lp_staking_instantiate_msg) =
        instantiate_lp_staking_contract(app, owner.clone(), mars_token_instance.clone());

    return (
        mars_token_instance,
        lp_staking_instance,
        staking_token_instance,
        lp_staking_instantiate_msg,
    );
}

#[test]
fn test_proper_initialization() {
    let owner = Addr::unchecked("contract_owner");

    let mut app = mock_app();
    let (
        mars_token_instance,
        lp_staking_instance,
        staking_token_instance,
        lp_staking_instantiate_msg,
    ) = init_all_contracts(&mut app, owner.clone());

    //  Should instantiate successfully

    // let's verify the config
    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(owner.to_string(), resp.owner);
    assert_eq!(mars_token_instance.to_string(), resp.mars_token);
    assert_eq!(staking_token_instance.to_string(), resp.staking_token);
    assert_eq!(
        lp_staking_instantiate_msg.init_timestamp.clone(),
        resp.init_timestamp
    );
    assert_eq!(
        lp_staking_instantiate_msg.till_timestamp.clone(),
        resp.till_timestamp
    );
    assert_eq!(
        lp_staking_instantiate_msg.cycle_duration,
        resp.cycle_duration
    );
    assert_eq!(
        lp_staking_instantiate_msg.reward_increase.unwrap(),
        resp.reward_increase
    );

    // let's verify the state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, resp.current_cycle);
    assert_eq!(Uint128::from(100000000u64), resp.current_cycle_rewards);
    assert_eq!(
        lp_staking_instantiate_msg.init_timestamp.clone(),
        resp.last_distributed
    );
    assert_eq!(Uint128::zero(), resp.total_bond_amount);
    assert_eq!(Decimal::zero(), resp.global_reward_index);
}

#[test]
fn test_update_config() {
    let owner = Addr::unchecked("contract_owner");

    let mut app = mock_app();
    let (mars_token_instance, lp_staking_instance, _, _) =
        init_all_contracts(&mut app, owner.clone());

    let update_msg = UpdateConfigMsg {
        owner: None,
        staking_token: Some("new_staking_token".to_string()),
        cycle_rewards: None,
        reward_increase: None,
    };

    // ######    ERROR :: Only owner can update configuration     ######
    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            lp_staking_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_msg.clone(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    // ######    ERROR :: Invalid reward increase ratio     ######
    let err = app
        .execute_contract(
            Addr::unchecked(owner.clone()),
            lp_staking_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: UpdateConfigMsg {
                    owner: None,
                    staking_token: Some("new_staking_token".to_string()),
                    cycle_rewards: None,
                    reward_increase: Some(Decimal::from_ratio(10u64, 1u64)),
                },
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Invalid reward increase ratio"
    );

    // ######    SUCCESS :: Should have successfully updated   ######

    app.execute_contract(
        Addr::unchecked(owner.clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: UpdateConfigMsg {
                owner: None,
                staking_token: Some("new_staking_token".to_string()),
                cycle_rewards: Some(Uint128::from(10000_000000u64)),
                reward_increase: Some(Decimal::from_ratio(10u64, 100u64)),
            },
        },
        &[],
    )
    .unwrap();

    // let's verify the config
    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(owner.to_string(), resp.owner);
    assert_eq!(mars_token_instance.to_string(), resp.mars_token);
    assert_eq!("new_staking_token".to_string(), resp.staking_token);
    assert_eq!(Decimal::from_ratio(10u64, 100u64), resp.reward_increase);

    // let's verify the state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, resp.current_cycle);
    assert_eq!(Uint128::from(10000_000000u64), resp.current_cycle_rewards);
}

#[test]
fn test_bond_tokens() {
    let owner = Addr::unchecked("contract_owner");

    let mut app = mock_app();
    let (mars_token_instance, lp_staking_instance, staking_token_instance, _) =
        init_all_contracts(&mut app, owner.clone());

    // ***
    // *** Test :: Staking before reward distribution goes live ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_03)
    });

    app.execute_contract(
        owner.clone(),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: "user".to_string(),
            amount: Uint128::new(1000_000000u128),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(1000u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, resp.current_cycle);
    assert_eq!(Uint128::from(1000u64), resp.total_bond_amount);
    assert_eq!(Decimal::zero(), resp.global_reward_index);

    // Check User State
    let user_resp: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u64), user_resp.bond_amount);
    assert_eq!(Decimal::zero(), user_resp.reward_index);
    assert_eq!(Uint128::from(0u64), user_resp.pending_reward);

    // ***
    // *** Test :: Staking when reward distribution goes live ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_13)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(1000u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, state_.current_cycle);
    assert_eq!(Uint128::from(100u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_13, state_.last_distributed);
    assert_eq!(Uint128::from(2000u64), state_.total_bond_amount);
    // assert_eq!(
    //     Decimal::from_ratio(30u64, 1000u64),
    //     state_.global_reward_index
    // );

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2000u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(30u64, 1000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(30u64), user_position_.pending_reward);

    // ***
    // *** Test :: Staking when reward distribution is live (within a block) ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_19)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(10u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, state_.current_cycle);
    assert_eq!(Uint128::from(100u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_19, state_.last_distributed);
    assert_eq!(Uint128::from(2010u64), state_.total_bond_amount);
    // assert_eq!(
    //     Decimal::from_ratio(60u64, 1000u64),
    //     state_.global_reward_index
    // );

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2010u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(60u64, 1000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(90u64), user_position_.pending_reward);

    // ***
    // *** Test :: Staking when reward distribution is live (spans multiple blocks) ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_47)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(70u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(3u64, state_.current_cycle);
    assert_eq!(Uint128::from(109u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_47, state_.last_distributed);
    assert_eq!(Uint128::from(2080u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2080u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(385u64), user_position_.pending_reward);

    // ***
    // *** Test :: Staking when reward distribution is about to be over ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_001_15)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(70u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(10u64, state_.current_cycle);
    assert_eq!(Uint128::from(0u64), state_.current_cycle_rewards);
    assert_eq!(1_000_001_10, state_.last_distributed);
    assert_eq!(Uint128::from(2150u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2150u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(1135u64), user_position_.pending_reward);

    // ***
    // *** Test :: Staking when reward distribution is over ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_001_31)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(30u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(10u64, state_.current_cycle);
    assert_eq!(Uint128::from(0u64), state_.current_cycle_rewards);
    assert_eq!(1_000_001_10, state_.last_distributed);
    assert_eq!(Uint128::from(2180u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(2180u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(1135u64), user_position_.pending_reward);
}

#[test]
fn test_unbond_tokens() {
    let owner = Addr::unchecked("contract_owner");

    let mut app = mock_app();
    let (mars_token_instance, lp_staking_instance, staking_token_instance, _) =
        init_all_contracts(&mut app, owner.clone());

    app.execute_contract(
        owner.clone(),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: "user".to_string(),
            amount: Uint128::new(1000_000000u128),
        },
        &[],
    )
    .unwrap();

    // ***
    // *** Test :: Staking when reward distribution is live (within a block) ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_15)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(10000000u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, state_.current_cycle);
    assert_eq!(Uint128::from(100u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_15, state_.last_distributed);
    assert_eq!(Uint128::from(10000000u64), state_.total_bond_amount);
    assert_eq!(
        Decimal::from_ratio(0u64, 1000u64),
        state_.global_reward_index
    );

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(10000000u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(0u64, 1000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);

    // ***
    // *** Test :: "Cannot unbond more than bond amount" Error ***
    // ***
    let err = app
        .execute_contract(
            Addr::unchecked("user".clone()),
            lp_staking_instance.clone(),
            &ExecuteMsg::Unbond {
                amount: Uint128::from(10000001u64),
                withdraw_pending_reward: Some(false),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Cannot unbond more than bond amount"
    );

    // ***
    // *** Test :: UN-Staking when reward distribution is live & don't claim rewards (same block) ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_17)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Unbond {
            amount: Uint128::from(100u64),
            withdraw_pending_reward: Some(false),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, state_.current_cycle);
    assert_eq!(Uint128::from(100u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_17, state_.last_distributed);
    assert_eq!(Uint128::from(9999900u64), state_.total_bond_amount);
    assert_eq!(
        Decimal::from_ratio(20u64, 10000000u64),
        state_.global_reward_index
    );

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9999900u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(20u64, 10000000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(20u64), user_position_.pending_reward);

    // ***
    // *** Test :: UN-Staking when reward distribution is live & claim rewards (same block) ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_19)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Unbond {
            amount: Uint128::from(100u64),
            withdraw_pending_reward: Some(true),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(0u64, state_.current_cycle);
    assert_eq!(Uint128::from(100u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_19, state_.last_distributed);
    assert_eq!(Uint128::from(9999900u64), state_.total_bond_amount);
    assert_eq!(
        Decimal::from_ratio(40000200002u64, 10000000000000000u64),
        state_.global_reward_index
    );

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9999800u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(40000200002u64, 10000000000000000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(20u64), user_position_.pending_reward);

    // ***
    // *** Test :: UN-Staking when reward distribution is live & don't claim rewards (spans multiple blocks) ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_37)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Unbond {
            amount: Uint128::from(100u64),
            withdraw_pending_reward: Some(false),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(2u64, state_.current_cycle);
    assert_eq!(Uint128::from(106u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_37, state_.last_distributed);
    assert_eq!(Uint128::from(9999500u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9999500u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(188u64), user_position_.pending_reward);

    // ***
    // *** Test :: UN-Staking when reward distribution is live & claim rewards (spans multiple blocks) ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_39)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Unbond {
            amount: Uint128::from(100u64),
            withdraw_pending_reward: Some(true),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(2u64, state_.current_cycle);
    assert_eq!(Uint128::from(106u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_39, state_.last_distributed);
    assert_eq!(Uint128::from(9999400u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9999400u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);

    // ***
    // *** Test :: UN-Staking when reward distribution is just over & claim rewards ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_001_15)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Unbond {
            amount: Uint128::from(100u64),
            withdraw_pending_reward: Some(true),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(10u64, state_.current_cycle);
    assert_eq!(Uint128::from(0u64), state_.current_cycle_rewards);
    assert_eq!(1_000_001_10, state_.last_distributed);
    assert_eq!(Uint128::from(9999300u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9999300u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);

    // ***
    // *** Test :: UN-Staking when reward distribution is over & claim rewards ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_001_45)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Unbond {
            amount: Uint128::from(100u64),
            withdraw_pending_reward: Some(true),
        },
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(10u64, state_.current_cycle);
    assert_eq!(Uint128::from(0u64), state_.current_cycle_rewards);
    assert_eq!(1_000_001_10, state_.last_distributed);
    assert_eq!(Uint128::from(9999200u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(9999200u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);
}

#[test]
fn test_claim_rewards() {
    let owner = Addr::unchecked("contract_owner");

    let mut app = mock_app();
    let (mars_token_instance, lp_staking_instance, staking_token_instance, _) =
        init_all_contracts(&mut app, owner.clone());

    app.execute_contract(
        owner.clone(),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Mint {
            recipient: "user".to_string(),
            amount: Uint128::new(1000_000000u128),
        },
        &[],
    )
    .unwrap();

    // ***
    // *** Test :: Staking before reward distribution goes live ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_03)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        staking_token_instance.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: lp_staking_instance.clone().to_string(),
            amount: Uint128::new(1000u128),
            msg: to_binary(&Cw20HookMsg::Bond {}).unwrap(),
        },
        &[],
    )
    .unwrap();

    // ***
    // *** Test #1 :: Claim Rewards  ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_23)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(1u64, state_.current_cycle);
    assert_eq!(Uint128::from(103u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_23, state_.last_distributed);
    assert_eq!(Uint128::from(1000u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(0u64, 1000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);

    // ***
    // *** Test #2 :: Claim Rewards  ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_000_73)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(6u64, state_.current_cycle);
    assert_eq!(Uint128::from(118u64), state_.current_cycle_rewards);
    assert_eq!(1_000_000_73, state_.last_distributed);
    assert_eq!(Uint128::from(1000u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u64), user_position_.bond_amount);
    assert_eq!(
        Decimal::from_ratio(0u64, 1000u64),
        user_position_.reward_index
    );
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);

    // ***
    // *** Test #3:: Claim Rewards  ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_001_13)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(10u64, state_.current_cycle);
    assert_eq!(Uint128::from(0u64), state_.current_cycle_rewards);
    assert_eq!(1_000_001_10, state_.last_distributed);
    assert_eq!(Uint128::from(1000u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);

    // ***
    // *** Test #4:: Claim Rewards  ***
    // ***

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(1_000_001_53)
    });

    app.execute_contract(
        Addr::unchecked("user".clone()),
        lp_staking_instance.clone(),
        &ExecuteMsg::Claim {},
        &[],
    )
    .unwrap();

    // Check Global State
    let state_: StateResponse = app
        .wrap()
        .query_wasm_smart(&lp_staking_instance, &QueryMsg::State { timestamp: None })
        .unwrap();
    assert_eq!(10u64, state_.current_cycle);
    assert_eq!(Uint128::from(0u64), state_.current_cycle_rewards);
    assert_eq!(1_000_001_10, state_.last_distributed);
    assert_eq!(Uint128::from(1000u64), state_.total_bond_amount);

    // Check User State
    let user_position_: StakerInfoResponse = app
        .wrap()
        .query_wasm_smart(
            &lp_staking_instance,
            &QueryMsg::StakerInfo {
                staker: "user".to_string(),
                timestamp: None,
            },
        )
        .unwrap();
    assert_eq!(Uint128::from(1000u64), user_position_.bond_amount);
    assert_eq!(Uint128::from(0u64), user_position_.pending_reward);
}
