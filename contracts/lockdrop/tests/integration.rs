use cosmwasm_std::testing::{mock_env, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{attr, to_binary, Addr, Coin, Decimal, Timestamp, Uint128, Uint64};
use cw20_base::msg::ExecuteMsg as CW20ExecuteMsg;
use mars_periphery::lockdrop::{
    CallbackMsg, ConfigResponse, ExecuteMsg, InstantiateMsg, LockUpInfoResponse, QueryMsg,
    StateResponse, UpdateConfigMsg, UserInfoResponse,
};
use terra_multi_test::{App, BankKeeper, ContractWrapper, Executor, TerraMockQuerier};

fn mock_app() -> App {
    let api = MockApi::default();
    let env = mock_env();
    let bank = BankKeeper::new();
    let storage = MockStorage::new();
    let tmq = TerraMockQuerier::new(MockQuerier::new(&[]));

    App::new(api, env.block, bank, storage, tmq)
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
        name: String::from("MARS token"),
        symbol: String::from("MARS"),
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
            String::from("MARS"),
            None,
        )
        .unwrap();
    mars_token_instance
}

// Instantiate Red Bank Contract { return (mars_address_provider_instance, red_bank_incentives_instance, red_bank_xmars_instance) }
fn instantiate_red_bank(app: &mut App, owner: Addr) -> (Addr, Addr, Addr) {
    // RED BANK :: Address provider
    let mars_address_provider = Box::new(ContractWrapper::new(
        mars_address_provider::contract::execute,
        mars_address_provider::contract::instantiate,
        mars_address_provider::contract::query,
    ));

    let mars_address_provider_code_id = app.store_code(mars_address_provider);

    let mars_address_provider_instance = app
        .instantiate_contract(
            mars_address_provider_code_id,
            owner.clone(),
            &mars_core::address_provider::msg::InstantiateMsg {
                owner: owner.clone().to_string(),
            },
            &[],
            String::from("address_provider"),
            None,
        )
        .unwrap();

    // RED BANK :: Staking Contract
    let mars_staking = Box::new(ContractWrapper::new(
        mars_staking::contract::execute,
        mars_staking::contract::instantiate,
        mars_staking::contract::query,
    ));

    let mars_staking_code_id = app.store_code(mars_staking);

    let mars_staking_instance = app
        .instantiate_contract(
            mars_staking_code_id,
            owner.clone(),
            &mars_core::staking::msg::InstantiateMsg {
                config: mars_core::staking::msg::CreateOrUpdateConfig {
                    owner: Some(owner.clone().to_string()),
                    address_provider_address: Some(
                        mars_address_provider_instance.clone().to_string(),
                    ),
                    astroport_factory_address: None,
                    astroport_max_spread: None,
                    cooldown_duration: None,
                },
            },
            &[],
            String::from("mars_staking"),
            None,
        )
        .unwrap();

    // RED BANK :: Incentives Contract
    let red_bank_incentives = Box::new(ContractWrapper::new(
        mars_incentives::contract::execute,
        mars_incentives::contract::instantiate,
        mars_incentives::contract::query,
    ));

    let red_bank_incentives_code_id = app.store_code(red_bank_incentives);

    let red_bank_incentives_instance = app
        .instantiate_contract(
            red_bank_incentives_code_id,
            owner.clone(),
            &mars_core::incentives::msg::InstantiateMsg {
                owner: owner.clone().to_string(),
                address_provider_address: mars_address_provider_instance.clone().to_string(),
            },
            &[],
            String::from("red_bank_incentives"),
            None,
        )
        .unwrap();

    // RED BANK :: XMARS Contract
    let red_bank_xmars = Box::new(ContractWrapper::new(
        mars_xmars_token::contract::execute,
        mars_xmars_token::contract::instantiate,
        mars_xmars_token::contract::query,
    ));

    let red_bank_xmars_code_id = app.store_code(red_bank_xmars);

    let red_bank_xmars_instance = app
        .instantiate_contract(
            red_bank_xmars_code_id,
            owner.clone(),
            &mars_core::xmars_token::msg::InstantiateMsg {
                name: String::from("XMARS token"),
                symbol: String::from("XMARS"),
                decimals: 6,
                initial_balances: vec![],
                mint: Some(cw20::MinterResponse {
                    minter: owner.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            String::from("xmars_token"),
            None,
        )
        .unwrap();

    return (
        mars_address_provider_instance,
        red_bank_incentives_instance,
        red_bank_xmars_instance,
    );
}

// Instantiate AUCTION Contract
fn instantiate_auction_contract(
    app: &mut App,
    owner: Addr,
    mars_token_instance: Addr,
    airdrop_instance: Addr,
    lockdrop_instance: Addr,
    pair_instance: Addr,
    lp_token_instance: Addr,
) -> (Addr, mars_periphery::auction::InstantiateMsg) {
    let auction_contract = Box::new(ContractWrapper::new(
        mars_auction::contract::execute,
        mars_auction::contract::instantiate,
        mars_auction::contract::query,
    ));

    let auction_code_id = app.store_code(auction_contract);

    let auction_instantiate_msg = mars_periphery::auction::InstantiateMsg {
        owner: owner.clone().to_string(),
        mars_token_address: mars_token_instance.clone().into_string(),
        astro_token_address: "astro_token".to_string(),
        airdrop_contract_address: airdrop_instance.to_string(),
        lockdrop_contract_address: lockdrop_instance.to_string(),
        generator_contract: "generator_contract".to_string(),
        mars_rewards: Uint128::from(1000000000000u64),
        mars_vesting_duration: 7776000u64,
        lp_tokens_vesting_duration: 7776000u64,
        init_timestamp: 1_000_00,
        deposit_window: 100_000_00,
        withdrawal_window: 5_000_00,
    };

    // Init contract
    let auction_instance = app
        .instantiate_contract(
            auction_code_id,
            owner.clone(),
            &auction_instantiate_msg,
            &[],
            "auction",
            None,
        )
        .unwrap();
    (auction_instance, auction_instantiate_msg)
}

// Instantiate LOCKDROP Contract
fn instantiate_lockdrop_contract(
    app: &mut App,
    owner: Addr,
    address_provider: Addr,
    auction_contract_address: Addr,
    ma_ust_token: Addr,
) -> (Addr, InstantiateMsg) {
    let lockdrop_contract = Box::new(ContractWrapper::new(
        mars_lockdrop::contract::execute,
        mars_lockdrop::contract::instantiate,
        mars_lockdrop::contract::query,
    ));

    let lockdrop_code_id = app.store_code(lockdrop_contract);

    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_00)
    });

    let lockdrop_instantiate_msg = mars_periphery::lockdrop::InstantiateMsg {
        owner: owner.clone().to_string(),
        address_provider: Some(address_provider.to_string()),
        auction_contract_address: Some(auction_contract_address.to_string()),
        ma_ust_token: Some(ma_ust_token.to_string()),
        init_timestamp: 10_000_01,
        deposit_window: 5_000_00,
        withdrawal_window: 2_000_00,
        min_duration: 2,
        max_duration: 51,
        seconds_per_week: 7 * 86400 as u64,
        weekly_multiplier: 9u64,
        weekly_divider: 100u64,
        lockdrop_incentives: Uint128::from(1000000000000u64),
    };

    // Init contract
    let lockdrop_instance = app
        .instantiate_contract(
            lockdrop_code_id,
            owner.clone(),
            &lockdrop_instantiate_msg,
            &[],
            "auction",
            None,
        )
        .unwrap();
    (lockdrop_instance, lockdrop_instantiate_msg)
}

// Initiates Lockdrop Contract with properly configured Config
// fn init_all_contracts(app: &mut App, owner: Addr) -> (Addr, Addr, Addr, Addr, InstantiateMsg) {
//     let mars_token_instance = instantiate_mars_token(app, owner.clone());

//     let (lp_staking_instance, staking_token_instance, lp_staking_instantiate_msg) =
//         instantiate_lp_staking_contract(app, owner.clone(), mars_token_instance.clone());

//     return (
//         mars_token_instance,
//         lp_staking_instance,
//         staking_token_instance,
//         lp_staking_instantiate_msg,
//     );
// }

#[test]
fn test_proper_initialization() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (lockdrop_instance, init_msg) = instantiate_lockdrop_contract(
        &mut app,
        owner,
        Addr::unchecked("address_provider"),
        Addr::unchecked("auction_contract_address"),
        Addr::unchecked("ma_ust_token"),
    );

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config
    assert_eq!(init_msg.owner, resp.owner);
    assert_eq!(init_msg.address_provider.unwrap(), resp.address_provider);
    assert_eq!(init_msg.ma_ust_token.unwrap(), resp.ma_ust_token);
    assert_eq!(
        init_msg.auction_contract_address.unwrap(),
        resp.auction_contract_address
    );
    assert_eq!(init_msg.init_timestamp, resp.init_timestamp);
    assert_eq!(init_msg.deposit_window, resp.deposit_window);
    assert_eq!(init_msg.withdrawal_window, resp.withdrawal_window);
    assert_eq!(init_msg.min_duration, resp.min_duration);
    assert_eq!(init_msg.max_duration, resp.max_duration);
    assert_eq!(init_msg.weekly_multiplier, resp.weekly_multiplier);
    assert_eq!(init_msg.weekly_divider, resp.weekly_divider);
    assert_eq!(init_msg.lockdrop_incentives, resp.lockdrop_incentives);

    // Check state
    let resp: StateResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::State {})
        .unwrap();

    assert_eq!(Uint128::zero(), resp.final_ust_locked);
    assert_eq!(Uint128::zero(), resp.final_maust_locked);
    assert_eq!(Uint128::zero(), resp.total_ust_locked);
    assert_eq!(Uint128::zero(), resp.total_maust_locked);
    assert_eq!(Uint128::zero(), resp.total_mars_delegated);
    assert_eq!(false, resp.are_claims_allowed);
    assert_eq!(Uint128::zero(), resp.total_deposits_weight);
    assert_eq!(Decimal::zero(), resp.xmars_rewards_index);
}

#[test]
fn update_config() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");

    let (lockdrop_instance, _) = instantiate_lockdrop_contract(
        &mut app,
        owner.clone(),
        Addr::unchecked("address_provider"),
        Addr::unchecked("auction_contract_address"),
        Addr::unchecked("ma_ust_token"),
    );

    let update_config = UpdateConfigMsg {
        owner: Some("new_owner".to_string()),
        address_provider: Some("new_address_provider".to_string()),
        ma_ust_token: Some("new_ma_ust_token".to_string()),
        auction_contract_address: Some("new_auction_contract".to_string()),
    };

    // ******* Error ::: Only owner can update *******

    let err = app
        .execute_contract(
            Addr::unchecked("wrong_owner"),
            lockdrop_instance.clone(),
            &ExecuteMsg::UpdateConfig {
                new_config: update_config.clone(),
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.to_string(),
        "Generic error: Only owner can update configuration"
    );

    // ******* Successfully update  *******

    app.execute_contract(
        owner,
        lockdrop_instance.clone(),
        &ExecuteMsg::UpdateConfig {
            new_config: update_config,
        },
        &[],
    )
    .unwrap();

    let resp: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&lockdrop_instance, &QueryMsg::Config {})
        .unwrap();

    // Check config and make sure all fields are updated
    assert_eq!("new_owner".to_string(), resp.owner);
    assert_eq!("new_address_provider".to_string(), resp.address_provider);
    assert_eq!("new_ma_ust_token".to_string(), resp.ma_ust_token);
    assert_eq!(
        "new_auction_contract".to_string(),
        resp.auction_contract_address
    );
}

#[test]
fn test_deposit_ust() {
    let mut app = mock_app();
    let owner = Addr::unchecked("contract_owner");
    let (lockdrop_instance, _) = instantiate_lockdrop_contract(
        &mut app,
        owner.clone(),
        Addr::unchecked("address_provider"),
        Addr::unchecked("auction_contract_address"),
        Addr::unchecked("ma_ust_token"),
    );

    let user1_address = Addr::unchecked("user1");
    let user2_address = Addr::unchecked("user2");
    // Set user balances
    app.init_bank_balance(
        &user1_address.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(20000000u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(20000000u128),
            },
        ],
    )
    .unwrap();
    app.init_bank_balance(
        &user2_address.clone(),
        vec![
            Coin {
                denom: "uusd".to_string(),
                amount: Uint128::new(5435435u128),
            },
            Coin {
                denom: "uluna".to_string(),
                amount: Uint128::new(20000000u128),
            },
        ],
    )
    .unwrap();

    // ***
    // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt before deposit window is open ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // ***
    // *** Test :: Error "Deposit window closed" Reason :: Deposit attempt after deposit window is closed ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(15_000_03)
    });
    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[],
        )
        .unwrap_err();
    assert_eq!(err.to_string(), "Generic error: Deposit window closed");

    // ***
    // *** Test :: Error "Trying to deposit several coins" ***
    // ***
    app.update_block(|b| {
        b.height += 17280;
        b.time = Timestamp::from_seconds(10_000_03)
    });

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[
                Coin {
                    denom: "uusd".to_string(),
                    amount: Uint128::from(432423u128),
                },
                Coin {
                    denom: "uluna".to_string(),
                    amount: Uint128::from(432423u128),
                },
            ],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Trying to deposit several coins"
    );

    // ***
    // *** Test :: Error "Only UST among native tokens accepted" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[Coin {
                denom: "uluna".to_string(),
                amount: Uint128::from(432423u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Only UST among native tokens accepted"
    );

    // ***
    // *** Test :: Error "Amount must be greater than 0" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 3u64 },
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(0u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Amount must be greater than 0"
    );

    // ***
    // *** Test :: Error "Lockup duration needs to be between {} and {}" ***
    // ***

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 1u64 },
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(10u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Lockup duration needs to be between 2 and 51"
    );

    let err = app
        .execute_contract(
            user1_address.clone(),
            lockdrop_instance.clone(),
            &ExecuteMsg::DepositUst { duration: 52u64 },
            &[Coin {
                denom: "uusd".to_string(),
                amount: Uint128::from(10u128),
            }],
        )
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Generic error: Lockup duration needs to be between 2 and 51"
    );

    // ***
    // *** Test #1 :: Successfully deposit UST  ***
    // ***
    // deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
    // let mut deposit_s = execute(
    //     deps.as_mut(),
    //     env.clone(),
    //     info.clone(),
    //     deposit_msg.clone(),
    // )
    // .unwrap();
    // assert_eq!(
    //     deposit_s.attributes,
    //     vec![
    //         attr("action", "lockdrop::ExecuteMsg::LockUST"),
    //         attr("user", "depositor"),
    //         attr("duration", "3"),
    //         attr("ust_deposited", "10000")
    //     ]
    // );
    // // let's verify the Lockdrop
    // let mut lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
    // assert_eq!(3u64, lockdrop_.duration);
    // assert_eq!(Uint128::from(10000u64), lockdrop_.ust_locked);
    // assert_eq!(Uint128::zero(), lockdrop_.maust_balance);
    // assert_eq!(Uint128::from(21432423343u64), lockdrop_.lockdrop_reward);
    // assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
    // // let's verify the User
    // let mut user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
    // assert_eq!(Uint128::from(10000u64), user_.total_ust_locked);
    // assert_eq!(Uint128::zero(), user_.total_maust_locked);
    // assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
    // assert_eq!(false, user_.is_lockdrop_claimed);
    // assert_eq!(Decimal::zero(), user_.reward_index);
    // assert_eq!(Uint128::zero(), user_.pending_xmars);
    // // let's verify the state
    // let mut state_ = query_state(deps.as_ref()).unwrap();
    // assert_eq!(Uint128::zero(), state_.final_ust_locked);
    // assert_eq!(Uint128::zero(), state_.final_maust_locked);
    // assert_eq!(Uint128::from(10000u64), state_.total_ust_locked);
    // assert_eq!(Uint128::zero(), state_.total_maust_locked);
    // assert_eq!(Uint128::from(2700u64), state_.total_deposits_weight);

    // // ***
    // // *** Test #2 :: Successfully deposit UST  ***
    // // ***
    // info = cosmwasm_std::testing::mock_info("depositor", &[coin(100u128, "uusd")]);
    // deposit_s = execute(
    //     deps.as_mut(),
    //     env.clone(),
    //     info.clone(),
    //     deposit_msg.clone(),
    // )
    // .unwrap();
    // assert_eq!(
    //     deposit_s.attributes,
    //     vec![
    //         attr("action", "lockdrop::ExecuteMsg::LockUST"),
    //         attr("user", "depositor"),
    //         attr("duration", "3"),
    //         attr("ust_deposited", "100")
    //     ]
    // );
    // // let's verify the Lockdrop
    // lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
    // assert_eq!(3u64, lockdrop_.duration);
    // assert_eq!(Uint128::from(10100u64), lockdrop_.ust_locked);
    // assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
    // // let's verify the User
    // user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
    // assert_eq!(Uint128::from(10100u64), user_.total_ust_locked);
    // assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
    // // let's verify the state
    // state_ = query_state(deps.as_ref()).unwrap();
    // assert_eq!(Uint128::from(10100u64), state_.total_ust_locked);
    // assert_eq!(Uint128::from(2727u64), state_.total_deposits_weight);

    // // ***
    // // *** Test #3 :: Successfully deposit UST (new lockup)  ***
    // // ***
    // deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
    // info = cosmwasm_std::testing::mock_info("depositor", &[coin(5432u128, "uusd")]);
    // deposit_s = execute(
    //     deps.as_mut(),
    //     env.clone(),
    //     info.clone(),
    //     deposit_msg.clone(),
    // )
    // .unwrap();
    // assert_eq!(
    //     deposit_s.attributes,
    //     vec![
    //         attr("action", "lockdrop::ExecuteMsg::LockUST"),
    //         attr("user", "depositor"),
    //         attr("duration", "5"),
    //         attr("ust_deposited", "5432")
    //     ]
    // );
    // // let's verify the Lockdrop
    // lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
    // assert_eq!(5u64, lockdrop_.duration);
    // assert_eq!(Uint128::from(5432u64), lockdrop_.ust_locked);
    // assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
    // // let's verify the User
    // user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
    // assert_eq!(Uint128::from(15532u64), user_.total_ust_locked);
    // assert_eq!(
    //     vec!["depositor3".to_string(), "depositor5".to_string()],
    //     user_.lockup_position_ids
    // );
    // // let's verify the state
    // state_ = query_state(deps.as_ref()).unwrap();
    // assert_eq!(Uint128::from(15532u64), state_.total_ust_locked);
    // assert_eq!(Uint128::from(5171u64), state_.total_deposits_weight);
}

//     #[test]
//     fn test_withdraw_ust() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let info = cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Test :: Error "Withdrawals not allowed" Reason :: Withdrawal attempt after the window is closed ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(10_00_720_11),
//             ..Default::default()
//         });
//         let mut withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(100u64),
//             duration: 5u64,
//         };
//         let mut withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Withdrawals not allowed");

//         // ***
//         // *** Test :: Error "Lockup doesn't exist" Reason :: Invalid lockup ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(10_00_120_10),
//             ..Default::default()
//         });
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(100u64),
//             duration: 4u64,
//         };
//         withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Lockup doesn't exist");

//         // ***
//         // *** Test :: Error "Invalid withdrawal request" Reason :: Invalid amount ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(100000000u64),
//             duration: 5u64,
//         };
//         withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Invalid withdrawal request");

//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(0u64),
//             duration: 5u64,
//         };
//         withdrawal_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         );
//         assert_generic_error_message(withdrawal_f, "Invalid withdrawal request");

//         // ***
//         // *** Test #1 :: Successfully withdraw UST  ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(42u64),
//             duration: 5u64,
//         };
//         let mut withdrawal_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             withdrawal_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_withdrawn", "42")
//             ]
//         );
//         // let's verify the Lockdrop
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint128::from(999958u64), lockdrop_.ust_locked);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(1999958u64), user_.total_ust_locked);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );
//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::from(1999958u64), state_.total_ust_locked);
//         assert_eq!(Uint128::from(719982u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #2 :: Successfully withdraw UST  ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(999958u64),
//             duration: 5u64,
//         };
//         withdrawal_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             withdrawal_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_withdrawn", "999958")
//             ]
//         );
//         // let's verify the Lockdrop
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint128::from(0u64), lockdrop_.ust_locked);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(1000000u64), user_.total_ust_locked);
//         assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::from(1000000u64), state_.total_ust_locked);
//         assert_eq!(Uint128::from(270001u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #3 :: Successfully withdraw UST  ***
//         // ***
//         withdrawal_msg = ExecuteMsg::WithdrawUst {
//             amount: Uint128::from(1000u64),
//             duration: 3u64,
//         };
//         withdrawal_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             withdrawal_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             withdrawal_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::WithdrawUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_withdrawn", "1000")
//             ]
//         );
//         // let's verify the Lockdrop
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(3u64, lockdrop_.duration);
//         assert_eq!(Uint128::from(999000u64), lockdrop_.ust_locked);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(999000u64), user_.total_ust_locked);
//         assert_eq!(vec!["depositor3".to_string()], user_.lockup_position_ids);
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::from(999000u64), state_.total_ust_locked);
//         assert_eq!(Uint128::from(269731u64), state_.total_deposits_weight);
//     }

//     #[test]
//     fn test_deposit_ust_in_red_bank() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Test :: Error "Unauthorized" ***
//         // ***
//         let deposit_in_redbank_msg = ExecuteMsg::DepositUstInRedBank {};
//         let deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(deposit_in_redbank_response_f, "Unauthorized");

//         // ***
//         // *** Test :: Error "Lockdrop deposits haven't concluded yet" ***
//         // ***
//         info = mock_info("owner");
//         let mut deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(
//             deposit_in_redbank_response_f,
//             "Lockdrop deposits haven't concluded yet",
//         );

//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_09),
//             ..Default::default()
//         });
//         deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(
//             deposit_in_redbank_response_f,
//             "Lockdrop deposits haven't concluded yet",
//         );

//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_09),
//             ..Default::default()
//         });
//         deposit_in_redbank_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         );
//         assert_generic_error_message(
//             deposit_in_redbank_response_f,
//             "Lockdrop deposits haven't concluded yet",
//         );

//         // ***
//         // *** Successfully deposited ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_11),
//             ..Default::default()
//         });
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(0u128))],
//         );
//         let deposit_in_redbank_response_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_in_redbank_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_in_redbank_response_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
//                 attr("ust_deposited_in_red_bank", "2000000"),
//                 attr("timestamp", "100100011")
//             ]
//         );
//         assert_eq!(
//             deposit_in_redbank_response_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "red_bank".to_string(),
//                     msg: to_binary(&mars_core::red_bank::msg::ExecuteMsg::DepositNative {
//                         denom: "uusd".to_string(),
//                     })
//                     .unwrap(),
//                     funds: vec![Coin {
//                         denom: "uusd".to_string(),
//                         amount: Uint128::from(1999900u128),
//                     }]
//                 })),
//                 SubMsg::new(
//                     CallbackMsg::UpdateStateOnRedBankDeposit {
//                         prev_ma_ust_balance: Uint128::from(0u64)
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//             ]
//         );
//         // let's verify the state
//         let state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::zero(), state_.final_ust_locked);
//         assert_eq!(Uint128::zero(), state_.final_maust_locked);
//         assert_eq!(Uint128::from(2000000u64), state_.total_ust_locked);
//         assert_eq!(Uint128::zero(), state_.total_maust_locked);
//         assert_eq!(Decimal::zero(), state_.global_reward_index);
//         assert_eq!(Uint128::from(720000u64), state_.total_deposits_weight);
//     }

//     #[test]
//     fn test_update_state_on_red_bank_deposit_callback() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(197000u128),
//             )],
//         );

//         // ***** Setup *****

//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Successfully updates the state post deposit in Red Bank ***
//         // ***
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint128::from(100u64),
//         });
//         let redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::RedBankDeposit"),
//                 attr("maUST_minted", "196900")
//             ]
//         );

//         // let's verify the state
//         let state_ = query_state(deps.as_ref()).unwrap();
//         // final : tracks Total UST deposited / Total MA-UST Minted
//         assert_eq!(Uint128::from(2000000u64), state_.final_ust_locked);
//         assert_eq!(Uint128::from(196900u64), state_.final_maust_locked);
//         // Total : tracks UST / MA-UST Available with the lockdrop contract
//         assert_eq!(Uint128::zero(), state_.total_ust_locked);
//         assert_eq!(Uint128::from(196900u64), state_.total_maust_locked);
//         // global_reward_index, total_deposits_weight :: Used for lockdrop / X-Mars distribution
//         assert_eq!(Decimal::zero(), state_.global_reward_index);
//         assert_eq!(Uint128::from(720000u64), state_.total_deposits_weight);

//         // let's verify the User
//         let user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(2000000u64), user_.total_ust_locked);
//         assert_eq!(Uint128::from(196900u64), user_.total_maust_locked);
//         assert_eq!(false, user_.is_lockdrop_claimed);
//         assert_eq!(Decimal::zero(), user_.reward_index);
//         assert_eq!(Uint128::zero(), user_.pending_xmars);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );

//         // let's verify the lockup #1
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(3u64, lockdrop_.duration);
//         assert_eq!(Uint128::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(98450u64), lockdrop_.maust_balance);
//         assert_eq!(Uint128::from(8037158753u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);

//         // let's verify the lockup #2
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(5u64, lockdrop_.duration);
//         assert_eq!(Uint128::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(98450u64), lockdrop_.maust_balance);
//         assert_eq!(Uint128::from(13395264589u64), lockdrop_.lockdrop_reward);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//     }

//     #[test]
//     fn test_try_claim() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // ***
//         // *** Test :: Error "Claim not allowed" ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_09),
//             ..Default::default()
//         });
//         let claim_rewards_msg = ExecuteMsg::ClaimRewards {};
//         let mut claim_rewards_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         );
//         assert_generic_error_message(claim_rewards_response_f, "Claim not allowed");

//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_000_09),
//             ..Default::default()
//         });
//         claim_rewards_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         );
//         assert_generic_error_message(claim_rewards_response_f, "Claim not allowed");

//         // ***
//         // *** Test :: Error "No lockup to claim rewards for" ***
//         // ***
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_001_001_09),
//             ..Default::default()
//         });
//         info = mock_info("not_depositor");
//         claim_rewards_response_f = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         );
//         assert_generic_error_message(claim_rewards_response_f, "No lockup to claim rewards for");

//         // ***
//         // *** Test #1 :: Successfully Claim Rewards ***
//         // ***
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(100u64));
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(0u128))],
//         );
//         info = mock_info("depositor");
//         let mut claim_rewards_response_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             claim_rewards_response_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::ClaimRewards"),
//                 attr("unclaimed_xMars", "100")
//             ]
//         );
//         assert_eq!(
//             claim_rewards_response_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "incentives".to_string(),
//                     msg: to_binary(&mars_core::incentives::msg::ExecuteMsg::ClaimRewards {})
//                         .unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(
//                     CallbackMsg::UpdateStateOnClaim {
//                         user: Addr::unchecked("depositor".to_string()),
//                         prev_xmars_balance: Uint128::from(0u64)
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//             ]
//         );

//         // ***
//         // *** Test #2 :: Successfully Claim Rewards (doesn't claim XMars as no rewards to claim) ***
//         // ***
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(58460u128))],
//         );
//         claim_rewards_response_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             claim_rewards_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             claim_rewards_response_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::ClaimRewards"),
//                 attr("unclaimed_xMars", "0")
//             ]
//         );
//         assert_eq!(
//             claim_rewards_response_s.messages,
//             vec![SubMsg::new(
//                 CallbackMsg::UpdateStateOnClaim {
//                     user: Addr::unchecked("depositor".to_string()),
//                     prev_xmars_balance: Uint128::from(58460u64)
//                 }
//                 .to_cosmos_msg(&env.clone().contract.address)
//                 .unwrap()
//             ),]
//         );
//     }

//     #[test]
//     fn test_update_state_on_claim() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });
//         // Create some lockdrop positions for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         info = cosmwasm_std::testing::mock_info("depositor2", &[coin(6450000u128, "uusd")]);
//         deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor2"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "6450000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor2"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "6450000")
//             ]
//         );

//         // *** Successfully updates the state post deposit in Red Bank ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(197000u128),
//             )],
//         );
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint128::from(0u64),
//         });
//         let redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::RedBankDeposit"),
//                 attr("maUST_minted", "197000")
//             ]
//         );

//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         // final : tracks Total UST deposited / Total MA-UST Minted
//         assert_eq!(Uint128::from(14900000u64), state_.final_ust_locked);
//         assert_eq!(Uint128::from(197000u64), state_.final_maust_locked);
//         // Total : tracks UST / MA-UST Available with the lockdrop contract
//         assert_eq!(Uint128::zero(), state_.total_ust_locked);
//         assert_eq!(Uint128::from(197000u64), state_.total_maust_locked);
//         // global_reward_index, total_deposits_weight :: Used for lockdrop / X-Mars distribution
//         assert_eq!(Decimal::zero(), state_.global_reward_index);
//         assert_eq!(Uint128::from(5364000u64), state_.total_deposits_weight);

//         // ***
//         // *** Test #1 :: Successfully updates state on Reward claim (Claims both MARS and XMARS) ***
//         // ***

//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(Addr::unchecked(MOCK_CONTRACT_ADDR), Uint128::new(58460u128))],
//         );
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("mars_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(54568460u128),
//             )],
//         );

//         info = mock_info(&env.clone().contract.address.to_string());
//         let mut callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
//             user: Addr::unchecked("depositor".to_string()),
//             prev_xmars_balance: Uint128::from(100u64),
//         });
//         let mut redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
//                 attr("total_xmars_claimed", "58360"),
//                 attr("user", "depositor"),
//                 attr("mars_claimed", "2876835347"),
//                 attr("xmars_claimed", "7833")
//             ]
//         );
//         assert_eq!(
//             redbank_callback_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "mars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor".to_string(),
//                         amount: Uint128::from(2876835347u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "xmars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor".to_string(),
//                         amount: Uint128::from(7833u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//             ]
//         );
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::zero(), state_.total_ust_locked);
//         assert_eq!(
//             Decimal::from_ratio(58360u64, 197000u64),
//             state_.global_reward_index
//         );
//         // let's verify the User
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(2000000u64), user_.total_ust_locked);
//         assert_eq!(Uint128::from(26442u64), user_.total_maust_locked);
//         assert_eq!(true, user_.is_lockdrop_claimed);
//         assert_eq!(
//             Decimal::from_ratio(58360u64, 197000u64),
//             user_.reward_index
//         );
//         assert_eq!(Uint128::zero(), user_.pending_xmars);
//         assert_eq!(
//             vec!["depositor3".to_string(), "depositor5".to_string()],
//             user_.lockup_position_ids
//         );
//         // // let's verify user's lockup #1
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(Uint128::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(13221u64), lockdrop_.maust_balance);
//         assert_eq!(Uint128::from(1078813255u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // // let's verify user's lockup #1
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(Uint128::from(1000000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(13221u64), lockdrop_.maust_balance);
//         assert_eq!(Uint128::from(1798022092u64), lockdrop_.lockdrop_reward);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);

//         // ***
//         // *** Test #2 :: Successfully updates state on Reward claim (Claims only XMARS) ***
//         // ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(43534460u128),
//             )],
//         );
//         callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
//             user: Addr::unchecked("depositor".to_string()),
//             prev_xmars_balance: Uint128::from(56430u64),
//         });
//         redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
//                 attr("total_xmars_claimed", "43478030"),
//                 attr("user", "depositor"),
//                 attr("mars_claimed", "0"),
//                 attr("xmars_claimed", "5835767")
//             ]
//         );
//         assert_eq!(
//             redbank_callback_s.messages,
//             vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                 contract_addr: "xmars_token".to_string(),
//                 msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                     recipient: "depositor".to_string(),
//                     amount: Uint128::from(5835767u128),
//                 })
//                 .unwrap(),
//                 funds: vec![]
//             })),]
//         );
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(true, user_.is_lockdrop_claimed);
//         assert_eq!(Uint128::zero(), user_.pending_xmars);

//         // ***
//         // *** Test #3 :: Successfully updates state on Reward claim (Claims MARS and XMARS for 2nd depositor) ***
//         // ***
//         callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnClaim {
//             user: Addr::unchecked("depositor2".to_string()),
//             prev_xmars_balance: Uint128::from(0u64),
//         });
//         redbank_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             redbank_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::CallbackMsg::ClaimRewards"),
//                 attr("total_xmars_claimed", "43534460"),
//                 attr("user", "depositor2"),
//                 attr("mars_claimed", "18555587994"),
//                 attr("xmars_claimed", "75383466")
//             ]
//         );
//         assert_eq!(
//             redbank_callback_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "mars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor2".to_string(),
//                         amount: Uint128::from(18555587994u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "xmars_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor2".to_string(),
//                         amount: Uint128::from(75383466u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//             ]
//         );
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor2".to_string()).unwrap();
//         assert_eq!(Uint128::from(12900000u64), user_.total_ust_locked);
//         assert_eq!(Uint128::from(170557u64), user_.total_maust_locked);
//         assert_eq!(true, user_.is_lockdrop_claimed);
//         assert_eq!(Uint128::zero(), user_.pending_xmars);
//         assert_eq!(
//             vec!["depositor23".to_string(), "depositor25".to_string()],
//             user_.lockup_position_ids
//         );
//         // // let's verify user's lockup #1
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor23".to_string()).unwrap();
//         assert_eq!(Uint128::from(6450000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(85278u64), lockdrop_.maust_balance);
//         assert_eq!(Uint128::from(6958345498u64), lockdrop_.lockdrop_reward);
//         assert_eq!(101914410u64, lockdrop_.unlock_timestamp);
//         // // let's verify user's lockup #1
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor25".to_string()).unwrap();
//         assert_eq!(Uint128::from(6450000u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(85278u64), lockdrop_.maust_balance);
//         assert_eq!(Uint128::from(11597242496u64), lockdrop_.lockdrop_reward);
//         assert_eq!(103124010u64, lockdrop_.unlock_timestamp);
//     }

//     #[test]
//     fn test_try_unlock_position() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let mut env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });

//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // *** Successfully updates the state post deposit in Red Bank ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(19700000u128),
//             )],
//         );
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint128::from(0u64),
//         });
//         execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();

//         // ***
//         // *** Test :: Error "Invalid lockup" ***
//         // ***
//         let mut unlock_msg = ExecuteMsg::Unlock { duration: 4u64 };
//         let mut unlock_f = execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone());
//         assert_generic_error_message(unlock_f, "Invalid lockup");

//         // ***
//         // *** Test :: Error "{} seconds to Unlock" ***
//         // ***
//         info = mock_info("depositor");
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_040_95),
//             ..Default::default()
//         });
//         unlock_msg = ExecuteMsg::Unlock { duration: 3u64 };
//         unlock_f = execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone());
//         assert_generic_error_message(unlock_f, "1910315 seconds to Unlock");

//         // ***
//         // *** Test :: Should unlock successfully ***
//         // ***
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(8706700u64));
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("xmars_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(19700000u128),
//             )],
//         );
//         env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_020_040_95),
//             ..Default::default()
//         });
//         let unlock_s =
//             execute(deps.as_mut(), env.clone(), info.clone(), unlock_msg.clone()).unwrap();
//         assert_eq!(
//             unlock_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::UnlockPosition"),
//                 attr("owner", "depositor"),
//                 attr("duration", "3"),
//                 attr("maUST_unlocked", "9850000")
//             ]
//         );
//         assert_eq!(
//             unlock_s.messages,
//             vec![
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "incentives".to_string(),
//                     msg: to_binary(&mars_core::incentives::msg::ExecuteMsg::ClaimRewards {})
//                         .unwrap(),
//                     funds: vec![]
//                 })),
//                 SubMsg::new(
//                     CallbackMsg::UpdateStateOnClaim {
//                         user: Addr::unchecked("depositor".to_string()),
//                         prev_xmars_balance: Uint128::from(19700000u64)
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//                 SubMsg::new(
//                     CallbackMsg::DissolvePosition {
//                         user: Addr::unchecked("depositor".to_string()),
//                         duration: 3u64
//                     }
//                     .to_cosmos_msg(&env.clone().contract.address)
//                     .unwrap()
//                 ),
//                 SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//                     contract_addr: "ma_ust_token".to_string(),
//                     msg: to_binary(&cw20::Cw20ExecuteMsg::Transfer {
//                         recipient: "depositor".to_string(),
//                         amount: Uint128::from(9850000u128),
//                     })
//                     .unwrap(),
//                     funds: vec![]
//                 })),
//             ]
//         );
//     }

//     #[test]
//     fn test_try_dissolve_position() {
//         let mut deps = th_setup(&[]);
//         let deposit_amount = 1000000u128;
//         let mut info =
//             cosmwasm_std::testing::mock_info("depositor", &[coin(deposit_amount, "uusd")]);
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));
//         // Set tax data
//         deps.querier.set_native_tax(
//             Decimal::from_ratio(1u128, 100u128),
//             &[(String::from("uusd"), Uint128::new(100u128))],
//         );
//         deps.querier
//             .set_incentives_address(Addr::unchecked("incentives".to_string()));

//         // ***** Setup *****

//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_15),
//             ..Default::default()
//         });

//         // Create a lockdrop position for testing
//         let mut deposit_msg = ExecuteMsg::DepositUst { duration: 3u64 };
//         let mut deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );
//         deposit_msg = ExecuteMsg::DepositUst { duration: 5u64 };
//         deposit_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             deposit_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             deposit_s.attributes,
//             vec![
//                 attr("action", "lockdrop::ExecuteMsg::LockUST"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//                 attr("ust_deposited", "1000000")
//             ]
//         );

//         // *** Successfully updates the state post deposit in Red Bank ***
//         deps.querier.set_cw20_balances(
//             Addr::unchecked("ma_ust_token".to_string()),
//             &[(
//                 Addr::unchecked(MOCK_CONTRACT_ADDR),
//                 Uint128::new(19700000u128),
//             )],
//         );
//         info = mock_info(&env.clone().contract.address.to_string());
//         let callback_msg = ExecuteMsg::Callback(CallbackMsg::UpdateStateOnRedBankDeposit {
//             prev_ma_ust_balance: Uint128::from(0u64),
//         });
//         execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_msg.clone(),
//         )
//         .unwrap();

//         // ***
//         // *** Test #1 :: Should successfully dissolve the position ***
//         // ***
//         let mut callback_dissolve_msg = ExecuteMsg::Callback(CallbackMsg::DissolvePosition {
//             user: Addr::unchecked("depositor".to_string()),
//             duration: 3u64,
//         });
//         let mut dissolve_position_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_dissolve_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             dissolve_position_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::Callback::DissolvePosition"),
//                 attr("user", "depositor"),
//                 attr("duration", "3"),
//             ]
//         );
//         // let's verify the state
//         let mut state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::from(2000000u64), state_.final_ust_locked);
//         assert_eq!(Uint128::from(19700000u64), state_.final_maust_locked);
//         assert_eq!(Uint128::from(9850000u64), state_.total_maust_locked);
//         assert_eq!(Uint128::from(720000u64), state_.total_deposits_weight);
//         // let's verify the User
//         deps.querier
//             .set_unclaimed_rewards("cosmos2contract".to_string(), Uint128::from(0u64));
//         let mut user_ =
//             query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(1000000u64), user_.total_ust_locked);
//         assert_eq!(Uint128::from(9850000u64), user_.total_maust_locked);
//         assert_eq!(vec!["depositor5".to_string()], user_.lockup_position_ids);
//         // let's verify user's lockup #1 (which is dissolved)
//         let mut lockdrop_ =
//             query_lockup_info_with_id(deps.as_ref(), "depositor3".to_string()).unwrap();
//         assert_eq!(Uint128::from(0u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(0u64), lockdrop_.maust_balance);

//         // ***
//         // *** Test #2 :: Should successfully dissolve the position ***
//         // ***
//         callback_dissolve_msg = ExecuteMsg::Callback(CallbackMsg::DissolvePosition {
//             user: Addr::unchecked("depositor".to_string()),
//             duration: 5u64,
//         });
//         dissolve_position_callback_s = execute(
//             deps.as_mut(),
//             env.clone(),
//             info.clone(),
//             callback_dissolve_msg.clone(),
//         )
//         .unwrap();
//         assert_eq!(
//             dissolve_position_callback_s.attributes,
//             vec![
//                 attr("action", "lockdrop::Callback::DissolvePosition"),
//                 attr("user", "depositor"),
//                 attr("duration", "5"),
//             ]
//         );
//         // let's verify the state
//         state_ = query_state(deps.as_ref()).unwrap();
//         assert_eq!(Uint128::from(2000000u64), state_.final_ust_locked);
//         assert_eq!(Uint128::from(19700000u64), state_.final_maust_locked);
//         assert_eq!(Uint128::from(0u64), state_.total_maust_locked);
//         assert_eq!(Uint128::from(720000u64), state_.total_deposits_weight);
//         // let's verify the User
//         user_ = query_user_info(deps.as_ref(), env.clone(), "depositor".to_string()).unwrap();
//         assert_eq!(Uint128::from(0u64), user_.total_ust_locked);
//         assert_eq!(Uint128::from(0u64), user_.total_maust_locked);
//         // let's verify user's lockup #1 (which is dissolved)
//         lockdrop_ = query_lockup_info_with_id(deps.as_ref(), "depositor5".to_string()).unwrap();
//         assert_eq!(Uint128::from(0u64), lockdrop_.ust_locked);
//         assert_eq!(Uint128::from(0u64), lockdrop_.maust_balance);
//     }

//     fn th_setup(contract_balances: &[Coin]) -> OwnedDeps<MockStorage, MockApi, MarsMockQuerier> {
//         let mut deps = mock_dependencies(contract_balances);
//         let info = mock_info("owner");
//         let env = mock_env(MockEnvParams {
//             block_time: Timestamp::from_seconds(1_000_000_00),
//             ..Default::default()
//         });
//         // Config with valid base params
//         let base_config = InstantiateMsg {
//             owner: "owner".to_string(),
//             address_provider: Some("address_provider".to_string()),
//             ma_ust_token: Some("ma_ust_token".to_string()),
//             init_timestamp: 1_000_000_10,
//             deposit_window: 100000u64,
//             withdrawal_window: 72000u64,
//             min_duration: 3u64,
//             max_duration: 9u64,
//             seconds_per_week: 7 * 86400 as u64,
//             denom: Some("uusd".to_string()),
//             weekly_multiplier: Some(Decimal::from_ratio(9u64, 100u64)),
//             lockdrop_incentives: Some(Uint128::from(21432423343u64)),
//         };
//         instantiate(deps.as_mut(), env, info, base_config).unwrap();
//         deps
//     }
// }
