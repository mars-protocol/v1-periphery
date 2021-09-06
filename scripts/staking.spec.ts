import chalk from "chalk";
import { join } from "path"
import { LocalTerra, Wallet } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployContract, transferCW20Tokens, getCW20Balance } from "./helpers/helpers.js";
import {staking_UpdateConfig, staking_IncreasePosition, staking_DecreasePosition, staking_ClaimRewards,
    staking_getConfig, staking_getState, staking_getPositionInfo, staking_getTimestamp }  from "./helpers/staking_helpers.js";
import {addressProvider_updateConfig, addressProvider_getAddress} from "./helpers/mock_helpers.js"; 
import { strict } from "yargs";

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

const ARTIFACTS_PATH = "../artifacts"
const terra = new LocalTerra();

const deployer = terra.wallets.test1;

const terra_user_1 = terra.wallets.test2;
const terra_user_2 = terra.wallets.test3;
const terra_user_3 = terra.wallets.test4;

let address_provider_contract_address: string;
let mars_token_address: string;
let lp_token_address: string;
let staking_contract_address: string;

const init_timestamp = parseInt((Date.now()/1000).toFixed(0) + 10000 )
const till_timestamp = init_timestamp + (86400 * 30)

//----------------------------------------------------------------------------------------
// Setup : Test
//----------------------------------------------------------------------------------------

async function setupTest() {

    // Deploy LP Token
    let lp_token_config = { "name": "Astro LP",
                            "symbol": "LPAstro",
                            "decimals": 6,
                            "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
                            "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
                           }
    lp_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  lp_token_config )
    console.log(chalk.green(`LP Token deployed successfully, address : ${chalk.cyan(lp_token_address)}`));
    
    // Deploy MARS Token
    let mars_token_config = { "name": "MARS",
                            "symbol": "MARS",
                            "decimals": 6,
                            "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
                            "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
                           }
    mars_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  mars_token_config )
    console.log(chalk.green(`$MARS Token deployed successfully, address : ${chalk.cyan(mars_token_address)}`));
    
    // Deploy Address Provider (Mock)
    address_provider_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'address_provider.wasm'),  {"owner": deployer.key.accAddress } )    
    await addressProvider_updateConfig(terra, deployer, address_provider_contract_address, { "update_config" : { "config" : { "mars_token_address": mars_token_address }  } });
    let address_response = await addressProvider_getAddress(terra, address_provider_contract_address, "MarsToken");
    expect(address_response).to.equal(mars_token_address);
    console.log(chalk.green(`Address provider Contract deployed successfully, address : ${chalk.cyan(address_provider_contract_address)}`));

    let staking_config = { "owner":  deployer.key.accAddress,
                          "address_provider": address_provider_contract_address,
                          "staking_token": lp_token_address,
                          "init_timestamp": init_timestamp,
                          "till_timestamp": till_timestamp, 
                          "cycle_rewards": "100000000", 
                          "cycle_duration": 10, 
                          "reward_increase": "0.02" 
                        } 
    
    staking_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'mars_lp_staking.wasm'),  staking_config )    
    const stakingConfigResponse = await staking_getConfig(terra, staking_contract_address);
      expect(stakingConfigResponse).to.deep.equal({
        owner : deployer.key.accAddress,
        address_provider: address_provider_contract_address,
        mars_token: mars_token_address,
        staking_token: lp_token_address,
        init_timestamp: init_timestamp,
        till_timestamp: till_timestamp,
        cycle_duration: 10, 
        reward_increase: "0.02" 
      });

    console.log(chalk.green(`Staking Contract deployed successfully, address : ${chalk.cyan(staking_contract_address)}`));

    var contract_mars_balance_before_transfer = await getCW20Balance(terra, mars_token_address, staking_contract_address);
    var deployer_mars_balance_before_transfer = await getCW20Balance(terra, mars_token_address, deployer.key.accAddress);

    await transferCW20Tokens(terra, deployer, mars_token_address, staking_contract_address, 2500000 * 10**6 );

    var contract_mars_balance_after_transfer = await getCW20Balance(terra, mars_token_address, staking_contract_address);
    var deployer_mars_balance_after_transfer = await getCW20Balance(terra, mars_token_address, deployer.key.accAddress);

    expect(Number(contract_mars_balance_after_transfer) - Number(contract_mars_balance_before_transfer)).to.equal(2500000 * 10**6);
    expect(Number(deployer_mars_balance_before_transfer) - Number(deployer_mars_balance_after_transfer)).to.equal(2500000 * 10**6);

    await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_1.key.accAddress, 500000 * 10**6)
    await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_2.key.accAddress, 500000 * 10**6)
    await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_3.key.accAddress, 500000 * 10**6)
}

//----------------------------------------------------------------------------------------
// (ADMIN FUNCTION) Update Config : Test
//----------------------------------------------------------------------------------------

async function testUpdateConfig() {
    process.stdout.write("Should update config info correctly... ");
    


    // Update cycle_rewards, cycle_duration, reward_increase parameters
    await staking_UpdateConfig(terra, deployer, staking_contract_address,{ "update_config" : {  "new_config" : {
                                                                                                  "cycle_rewards": "100000000",
                                                                                                  "reward_increase": "0.03"
                                                                                                  }
                                                                                             }                                                                                                                      
                                                                        });
    let stakingConfigResponse = await staking_getConfig(terra, staking_contract_address);
    let global_state = await staking_getState(terra, staking_contract_address, 0);
    expect(stakingConfigResponse).to.deep.equal({ owner : deployer.key.accAddress,
                                                  address_provider: address_provider_contract_address,
                                                  mars_token: mars_token_address,
                                                  staking_token: lp_token_address,
                                                  init_timestamp: init_timestamp,
                                                  till_timestamp: till_timestamp,
                                                  cycle_duration: 10, 
                                                  reward_increase: "0.03"
                                                });
    expect(global_state["current_cycle_rewards"]).to.equal("100000000");
    console.log(chalk.green("\nCycle_rewards and Reward_increase configuration parameters updated successfully"));   
    
    // Update till_timestamp, init_timestamp parameters
    let new_init_timestamp = Number(parseInt((Date.now()/1000).toFixed(0))) + 3;
    await staking_UpdateConfig(terra, deployer, staking_contract_address,{ "update_config" : {  "new_config" : {
                                                                                                    "init_timestamp": new_init_timestamp,
                                                                                                    "till_timestamp": init_timestamp + 86400
                                                                                                    }
                                                                                                }                                                                                                                        
    });
    console.log(" REWARDS INIT TIMESTAMP = " + new_init_timestamp.toString() )
    stakingConfigResponse = await staking_getConfig(terra, staking_contract_address);
    expect(stakingConfigResponse).to.deep.equal({ owner : deployer.key.accAddress,
                                                  address_provider: address_provider_contract_address,
                                                  mars_token: mars_token_address,
                                                  staking_token: lp_token_address,
                                                  init_timestamp: new_init_timestamp,
                                                  till_timestamp: init_timestamp + 86400,
                                                  cycle_duration: 10, 
                                                  reward_increase: "0.03"
                                                  });
    console.log(chalk.green("Staking Rewards init and ending timestamps configuration parameters updated successfully"));            
}

//----------------------------------------------------------------------------------------
// Staking : Increase staked LP Position : Test
//----------------------------------------------------------------------------------------

async function test_IncreasePosition(userWallet:Wallet, amount:number) {
    process.stdout.write( `Should increase staked position of terra user  ${chalk.cyan(userWallet.key.accAddress)} by ${(amount).toString()}... `);

    let global_state_before = await staking_getState(terra, staking_contract_address, 0);
    let global_bond_amount_before = global_state_before.total_bond_amount;
 
    let user_position_before = await staking_getPositionInfo(terra, staking_contract_address, userWallet.key.accAddress,0);
    let user_bond_amount_before = user_position_before.bond_amount;
    expect(user_position_before.staker).to.equal(userWallet.key.accAddress)

    var contract_lp_balance_before_increase = await getCW20Balance(terra, lp_token_address, staking_contract_address);
    var user_lp_balance_before_increase = await getCW20Balance(terra, lp_token_address, userWallet.key.accAddress);

    await staking_IncreasePosition(terra, userWallet, staking_contract_address, lp_token_address, amount);

    var contract_lp_balance_after_increase = await getCW20Balance(terra, lp_token_address, staking_contract_address);
    var user_lp_balance_after_increase = await getCW20Balance(terra, lp_token_address, userWallet.key.accAddress);

    let global_state_after = await staking_getState(terra, staking_contract_address, 0);
    let global_bond_amount_after = global_state_after.total_bond_amount;
    let timestampResponse = await staking_getTimestamp(terra, staking_contract_address)
    expect(global_state_after.last_distributed).to.equal( timestampResponse["timestamp"] )

    let user_position_after = await staking_getPositionInfo(terra, staking_contract_address, userWallet.key.accAddress,0);
    let user_bond_amount_after = user_position_after.bond_amount;
    expect(user_position_after.staker).to.equal(userWallet.key.accAddress)

    expect(Number(contract_lp_balance_after_increase) - Number(contract_lp_balance_before_increase)).to.equal(amount);
    expect(Number(user_lp_balance_before_increase) - Number(user_lp_balance_after_increase)).to.equal(amount);
    expect(Number(user_bond_amount_after) - Number(user_bond_amount_before)).to.equal(amount);
    expect(Number(global_bond_amount_after) - Number(global_bond_amount_before)).to.equal(amount);

    console.log(chalk.green( `\n Staked Position size increased successfully by ${(amount).toString()}... `));                        
}

//----------------------------------------------------------------------------------------
// Staking : Decrease staked LP Position : Test
//----------------------------------------------------------------------------------------

async function test_DecreasePosition(userWallet:Wallet, amount:number) {
  process.stdout.write( `Should decrease staked position (& claim accumulate rewards) of terra user  ${chalk.cyan(userWallet.key.accAddress)} by ${(amount).toString()}... `);

  let global_state_before = await staking_getState(terra, staking_contract_address, 0);
  let global_bond_amount_before = global_state_before.total_bond_amount;

  var contract_lp_balance_before_decrease = await getCW20Balance(terra, lp_token_address, staking_contract_address);
  var user_lp_balance_before_decrease = await getCW20Balance(terra, lp_token_address, userWallet.key.accAddress);

  var user_mars_balance_before = await getCW20Balance(terra, mars_token_address, userWallet.key.accAddress);

  let user_position_before = await staking_getPositionInfo(terra, staking_contract_address, userWallet.key.accAddress,0);
  let user_bond_amount_before = user_position_before.bond_amount;
  expect(user_position_before.staker).to.equal(userWallet.key.accAddress)

  await staking_DecreasePosition(terra, userWallet, staking_contract_address, mars_token_address, amount);

  var contract_lp_balance_after_decrease = await getCW20Balance(terra, lp_token_address, staking_contract_address);
  var user_lp_balance_after_decrease = await getCW20Balance(terra, lp_token_address, userWallet.key.accAddress);

  let global_state_after = await staking_getState(terra, staking_contract_address, 0);
  let global_bond_amount_after = global_state_after.total_bond_amount;
  let timestampResponse = await staking_getTimestamp(terra, staking_contract_address)
  expect(global_state_after.last_distributed).to.equal( timestampResponse["timestamp"] )

  let user_position_after = await staking_getPositionInfo(terra, staking_contract_address, userWallet.key.accAddress,0);
  let user_bond_amount_after = user_position_after.bond_amount;
  expect(user_position_after.staker).to.equal(userWallet.key.accAddress)
  expect(Number(user_position_after.pending_reward)).to.equal(0)
  var user_mars_balance_after = await getCW20Balance(terra, mars_token_address, userWallet.key.accAddress);

  expect(Number(contract_lp_balance_before_decrease) - Number(contract_lp_balance_after_decrease)).to.equal(amount);
  expect(Number(user_lp_balance_after_decrease) - Number(user_lp_balance_before_decrease)).to.equal(amount);
  expect(Number(user_bond_amount_before) - Number(user_bond_amount_after)).to.equal(amount);
  expect(Number(global_bond_amount_before) - Number(global_bond_amount_after)).to.equal(amount);
  let mars_claimed = Number(user_mars_balance_after) - Number(user_mars_balance_before);

  console.log(chalk.green( `\n Staked Position size decreased successfully by ${(amount).toString()}... `));                        
  console.log(chalk.green( `${(mars_claimed).toString()} MARS rewards were successfully claimed... `));                        
}



//----------------------------------------------------------------------------------------
// Staking : Claim Rewards : Test
//----------------------------------------------------------------------------------------

async function test_ClaimRewards(userWallet:Wallet) {
  process.stdout.write( `Should claim accrued MARS rewards for terra user  ${chalk.cyan(userWallet.key.accAddress)} ... `);

  await sleep(1000);

  // let global_state_before = await staking_getState(terra, staking_contract_address, 0);
  // let global_bond_amount_before = global_state_before.total_bond_amount;

  var user_mars_balance_before = await getCW20Balance(terra, mars_token_address, userWallet.key.accAddress);
  let user_position_before = await staking_getPositionInfo(terra, staking_contract_address, userWallet.key.accAddress,0);
  expect(user_position_before.staker).to.equal(userWallet.key.accAddress)

  await staking_ClaimRewards(terra, userWallet, staking_contract_address, mars_token_address);

  let global_state_after = await staking_getState(terra, staking_contract_address, 0);
  let timestampResponse = await staking_getTimestamp(terra, staking_contract_address)
  expect(global_state_after.last_distributed).to.equal( timestampResponse["timestamp"] )

  let user_position_after = await staking_getPositionInfo(terra, staking_contract_address, userWallet.key.accAddress,0);
  expect(user_position_after.staker).to.equal(userWallet.key.accAddress)
  expect(Number(user_position_after.pending_reward)).to.equal(0)
  var user_mars_balance_after = await getCW20Balance(terra, mars_token_address, userWallet.key.accAddress);

  let mars_claimed = Number(user_mars_balance_after) - Number(user_mars_balance_before);

  console.log(chalk.green( `${(mars_claimed).toString()} MARS rewards were successfully claimed... `));                        
}



function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
    console.log(chalk.yellow("\nStaking Test: Info"));
  
    console.log(`Deployer ::  ${chalk.cyan(deployer.key.accAddress)}`);

    console.log(`${chalk.cyan(terra_user_1.key.accAddress)} as user #1`);
    console.log(`${chalk.cyan(terra_user_2.key.accAddress)} as user #2`);
    console.log(`${chalk.cyan(terra_user_3.key.accAddress)} as user #3`);
  
    // Deploy the contracts
    console.log(chalk.yellow("\nStaking Test: Setup"));
    await setupTest();

    // UpdateConfig :: Test
    console.log(chalk.yellow("\nStaking Test: Update Configuration"));
    await testUpdateConfig();

    // Cw20ReceiveMsg::Bond :: Test
    console.log(chalk.yellow("\nStaking Test #: Bond LP Tokens (increase position)"));
    await test_IncreasePosition(terra_user_1, 34534 * 10**6 )

    console.log(chalk.yellow("\nStaking Test #: UnBond LP Tokens (decrease position)"));
    await test_DecreasePosition(terra_user_1, 34534 * 10**6 )

    // Cw20ReceiveMsg::Bond :: Test
    console.log(chalk.yellow("\nStaking Test #: Bond LP Tokens (increase position)"));
    await test_IncreasePosition(terra_user_2, 1343 * 10**6 )

    // Cw20ReceiveMsg::Bond :: Test
    console.log(chalk.yellow("\nStaking Test #: Bond LP Tokens (increase position)"));
    await test_IncreasePosition(terra_user_3, 43534 * 10**6 )

    // Unbond :: Test
    console.log(chalk.yellow("\nStaking Test #: UnBond LP Tokens (decrease position)"));
    await test_DecreasePosition(terra_user_2, 442 * 10**6 )

    // Unbond :: Test
    console.log(chalk.yellow("\nStaking Test #: UnBond LP Tokens (decrease position)"));
    await test_DecreasePosition(terra_user_3, 565 * 10**6 )


    // Claim :: Test
    console.log(chalk.yellow("\nStaking Test #: Claim Rewards"));
    await test_ClaimRewards(terra_user_2)

    // Claim :: Test
    console.log(chalk.yellow("\nStaking Test #: Claim Rewards"));
    await test_ClaimRewards(terra_user_3)

    console.log("");
  })();

