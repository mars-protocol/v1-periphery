import chalk from "chalk";
import { join } from "path"
import { LocalTerra, Wallet } from "@terra-money/terra.js";
import { expect } from "chai";
import { uploadContract, deployContract, transferCW20Tokens, getCW20Balance, queryContract } from "./helpers/helpers.js";
import {Lockdrop_update_config, Lockdrop_deposit_UST, Lockdrop_withdraw_UST, Lockdrop_claim_rewards, Lockdrop_unlock_deposit,
    Lockdrop_deposit_UST_in_RedBank, query_lockdrop_config, query_lockdrop_state, query_lockdrop_userInfo, 
    query_lockdrop_lockupInfo, query_lockdrop_lockupInfoWithId }  from "./helpers/lockdrop_helpers.js";
import {addressProvider_updateConfig, addressProvider_getAddress, setupUST_Market_RedBank, incentives_set_asset_incentive} from "./helpers/mock_helpers.js"; 
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
let xmars_token_address: string;
let incentives_address: string;
let red_bank_address: string;
let lockdrop_contract_address: string;
let lockdrop_init_timestamp: Number;


//----------------------------------------------------------------------------------------
// Setup : Test
//----------------------------------------------------------------------------------------

async function setupTest() {
    
    // Deploy MARS Token
    let mars_token_config = { "name": "MARS",
                            "symbol": "MARS",
                            "decimals": 6,
                            "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
                            "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
                           }
    mars_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  mars_token_config )
    console.log(chalk.green(`MARS Token deployed successfully, address : ${chalk.cyan(mars_token_address)}`));
    
   // Deploy ma Assets Token Code
    let maToken_code = await uploadContract(terra, deployer, join(ARTIFACTS_PATH, 'mock_ma_token.wasm') );
    console.log(chalk.green(`ma-Asset Code successfully uploaded, codeId : ${chalk.cyan(maToken_code)}`));


    // Deploy Address Provider (Mock)
    address_provider_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'address_provider.wasm'),  {"owner": deployer.key.accAddress } )    
    await addressProvider_updateConfig(terra, deployer, address_provider_contract_address, { "update_config" :  { "config" : { "mars_token_address": mars_token_address   } }});                                                                                                                
    let address_response = await addressProvider_getAddress(terra, address_provider_contract_address, "MarsToken");
    expect(address_response).to.equal(mars_token_address);
    console.log(chalk.green(`Address provider Contract deployed successfully, address : ${chalk.cyan(address_provider_contract_address)}`));


    // Deploy MARS Staking Contract (Mock)
    let mock_staking_config = { "config" : {    "owner":  deployer.key.accAddress,
                                                "address_provider_address": address_provider_contract_address,
                                                "terraswap_factory_address": "terra18qpjm4zkvqnpjpw0zn0tdr8gdzvt8au35v45xf",
                                                "terraswap_max_spread": "0.05",
                                                "cooldown_duration": 90,
                                                "unstake_window": 300        
                                            }
                                };
    const stakingContractAddress = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'mock_staking.wasm'), mock_staking_config);
    console.log(chalk.green(`Staking Contract deployed successfully, address : ${chalk.cyan(stakingContractAddress)}`));

    // Deploy XMARS Token
    let xmars_token_config = { "name": "X-MARS",
                            "symbol": "XMARS",
                            "decimals": 6,
                            "initial_balances": [], 
                            "mint": { "minter": stakingContractAddress },
                           }
    xmars_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  xmars_token_config )
    console.log(chalk.green(`XMARS Token deployed successfully, address : ${chalk.cyan(xmars_token_address)}`));

    
    // Deploy Incentives Contract (Mock)
    let mock_incentives_config = { "owner":  deployer.key.accAddress,  "address_provider_address": address_provider_contract_address};
    incentives_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'mock_incentives.wasm'), mock_incentives_config);
    console.log(chalk.green(`Incentives Contract deployed successfully, address : ${chalk.cyan(incentives_address)}`));
    

    // Deploy Red Bank (Mock)
    let mock_redBank_config = { "config": { "owner":  deployer.key.accAddress,
                                            "address_provider_address": address_provider_contract_address,
                                            "insurance_fund_fee_share": "0.05",
                                            "treasury_fee_share": "0.05",
                                            "ma_token_code_id": maToken_code,
                                            "close_factor": "0.5"
                                          } 
                              };
    red_bank_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'mock_red_bank.wasm'), mock_redBank_config);
    console.log(chalk.green(`Red Bank Contract deployed successfully, address : ${chalk.cyan(red_bank_address)}`));
    

    
    // Update address provider Config
    await addressProvider_updateConfig(terra, deployer, address_provider_contract_address, { "update_config" : { "config" : {  "incentives_address": incentives_address,
                                                                                                                                "staking_address": stakingContractAddress, 
                                                                                                                                "red_bank_address": red_bank_address,
                                                                                                                                "protocol_admin_address": deployer.key.accAddress,
                                                                                                                                "xmars_token_address": xmars_token_address
                                                                                                                            }  
                                                                                                                } 
                                                                                            });
    console.log(chalk.green(`Address Provider :: Config updated successfully`));

    // Initialize money market
    await setupUST_Market_RedBank(terra, deployer,  red_bank_address );
    console.log(chalk.green(`Red Bank :: UST money market initialized successfully`));
    
    let res = await queryContract( terra, "terra176sfzp99t86pvmnxun6qjva374ranpw2942j9v", { "market" : {"asset" : { "native": {"denom":"uusd"} }} }  );
    // console.log(res);
    
    await incentives_set_asset_incentive(terra, deployer, incentives_address, res["ma_token_address"], 1  );
    console.log(chalk.green(`Incentives successfully set for UST money market`));

    await testUpdateConfig( { "update_config": {"new_config": {"address_provider": address_provider_contract_address, "ma_ust_token": res["ma_token_address"]}} } )
    console.log(chalk.green(`Lockdrop Contract :: Configuration successfully updated`));

    // var contract_mars_balance_before_transfer = await getCW20Balance(terra, mars_token_address, lockdrop_contract_address);
    // var deployer_mars_balance_before_transfer = await getCW20Balance(terra, mars_token_address, deployer.key.accAddress);

    // await transferCW20Tokens(terra, deployer, mars_token_address, lockdrop_contract_address, 2500000 * 10**6 );

    // var contract_mars_balance_after_transfer = await getCW20Balance(terra, mars_token_address, lockdrop_contract_address);
    // var deployer_mars_balance_after_transfer = await getCW20Balance(terra, mars_token_address, deployer.key.accAddress);

    // expect(Number(contract_mars_balance_after_transfer) - Number(contract_mars_balance_before_transfer)).to.equal(2500000 * 10**6);
    // expect(Number(deployer_mars_balance_before_transfer) - Number(deployer_mars_balance_after_transfer)).to.equal(2500000 * 10**6);

    // await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_1.key.accAddress, 500000 * 10**6)
    // await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_2.key.accAddress, 500000 * 10**6)
    // await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_3.key.accAddress, 500000 * 10**6)
}



//----------------------------------------------------------------------------------------
// (ADMIN FUNCTION) Update Config : Test
//----------------------------------------------------------------------------------------

async function testUpdateConfig( newConfig: any ) {
    process.stdout.write("Should update config info correctly... ");
    
    await Lockdrop_update_config(terra, deployer, lockdrop_contract_address,newConfig);

    let config_Response = await query_lockdrop_config(terra, lockdrop_contract_address);

    expect(config_Response).to.deep.equal({       owner : deployer.key.accAddress,
                                                  address_provider: newConfig["new_config"]["address_provider"],
                                                  ma_ust_token: newConfig["new_config"]["ma_ust_token"],
                                                  init_timestamp:  lockdrop_init_timestamp,
                                                  deposit_window: 59,
                                                  withdrawal_window: 45,
                                                  min_duration: 1, 
                                                  max_duration: 5,
                                                  multiplier: "0.0769",
                                                  lockdrop_incentives: 5000000000000
                                                });
    console.log(chalk.green("\Lockdrop :: address_provider and ma_ust_token addresses parameters updated successfully"));   
}


//----------------------------------------------------------------------------------------
// Deposit UST : Test
//----------------------------------------------------------------------------------------


async function test_deposit_UST(userWallet:Wallet, amount:number, duration: number) {
    process.stdout.write( `Should increase deposited UST for terra user  ${chalk.cyan(userWallet.key.accAddress)} by ${(amount).toString()} for duration = ${(duration).toString()} weeks... `);

    let global_state_before = await query_lockdrop_state(terra, lockdrop_contract_address);
    let global_deposit_amount_before = global_state_before.total_ust_locked;
 
    let user_info_before = await query_lockdrop_userInfo(terra, lockdrop_contract_address, userWallet.key.accAddress);
    let user_deposit_amount_before = user_info_before.total_ust_locked;
    let user_deposit_position_ids_before = user_info_before.lockup_position_ids;

    var lockup_before = await query_lockdrop_lockupInfoWithId(terra, lockdrop_contract_address, userWallet.key.accAddress + duration.toString() );

    await Lockdrop_deposit_UST(terra, userWallet, lockdrop_contract_address, amount, duration);

    let global_state_after = await query_lockdrop_state(terra, lockdrop_contract_address);
    let global_deposit_amount_after = global_state_after.total_ust_locked;

    let user_info_after = await query_lockdrop_userInfo(terra, lockdrop_contract_address, userWallet.key.accAddress);
    let user_deposit_amount_after = user_info_after.total_ust_locked;
    let user_deposit_position_ids_after = user_info_after.lockup_position_ids;

    var lockup_after = await query_lockdrop_lockupInfoWithId(terra, lockdrop_contract_address, userWallet.key.accAddress + duration.toString() );
    expect(Number(lockup_after.duration)).to.equal(duration);

    expect(Number(global_deposit_amount_after) - Number(global_deposit_amount_before)).to.equal(amount);
    expect(Number(user_deposit_amount_after) - Number(user_deposit_amount_before)).to.equal(amount);

    console.log(chalk.green( `\n UST deposited successfully to the lockdrop contract ... \n`));                        
}

//----------------------------------------------------------------------------------------
// Withdraw UST : Test
//----------------------------------------------------------------------------------------


async function test_withdraw_UST(userWallet:Wallet, amount:number, duration: number) {
    process.stdout.write( `Should decrease deposited UST for terra user  ${chalk.cyan(userWallet.key.accAddress)} by ${(amount).toString()} for duration = ${(duration).toString()} weeks... `);

    let global_state_before = await query_lockdrop_state(terra, lockdrop_contract_address);
    let global_deposit_amount_before = global_state_before.total_ust_locked;
 
    let user_info_before = await query_lockdrop_userInfo(terra, lockdrop_contract_address, userWallet.key.accAddress);
    let user_deposit_amount_before = user_info_before.total_ust_locked;
    let user_deposit_position_ids_before = user_info_before.lockup_position_ids;

    var lockup_before = await query_lockdrop_lockupInfoWithId(terra, lockdrop_contract_address, userWallet.key.accAddress + duration.toString() );

    await Lockdrop_withdraw_UST(terra, userWallet, lockdrop_contract_address, amount, duration);

    let global_state_after = await query_lockdrop_state(terra, lockdrop_contract_address);
    let global_deposit_amount_after = global_state_after.total_ust_locked;

    let user_info_after = await query_lockdrop_userInfo(terra, lockdrop_contract_address, userWallet.key.accAddress);
    let user_deposit_amount_after = user_info_after.total_ust_locked;
    let user_deposit_position_ids_after = user_info_after.lockup_position_ids;

    var lockup_after = await query_lockdrop_lockupInfoWithId(terra, lockdrop_contract_address, userWallet.key.accAddress + duration.toString() );
    expect(Number(lockup_after.duration)).to.equal(duration);

    expect(Number(global_deposit_amount_before) - Number(global_deposit_amount_after)).to.equal(amount);
    expect(Number(user_deposit_amount_before) - Number(user_deposit_amount_after)).to.equal(amount);

    console.log(chalk.green( `\n UST withdrawn successfully from the lockdrop contract ... \n`));                        
}






























//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
    console.log(chalk.yellow("\Lockdrop Test: Info"));
  
    console.log(`Deployer ::  ${chalk.cyan(deployer.key.accAddress)}`);

    console.log(`${chalk.cyan(terra_user_1.key.accAddress)} as user #1`);
    console.log(`${chalk.cyan(terra_user_2.key.accAddress)} as user #2`);
    console.log(`${chalk.cyan(terra_user_3.key.accAddress)} as user #3`);
  
    lockdrop_init_timestamp = Number(parseInt((Date.now()/1000).toFixed(0))) + 7;

    // Deploy Lockdrop Contract
    let lockdrop_config = { "owner":  deployer.key.accAddress,
                                            "address_provider": undefined,
                                            "ma_ust_token": undefined,
                                            "init_timestamp": lockdrop_init_timestamp,
                                            "deposit_window": 59,
                                            "withdrawal_window": 45,
                                            "min_duration": 1,
                                            "max_duration": 5,
                                            "weekly_multiplier": "0.0769",
                                            "denom": "uusd",
                                            "lockdrop_incentives": "5000000000000"
                                          } ;
                            //   };
    lockdrop_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'lockdrop.wasm'), lockdrop_config);
    console.log(chalk.green(`Lockdrop Contract deployed successfully, address : ${chalk.cyan(lockdrop_contract_address)}`));
    
    await sleep(7000);
    await test_deposit_UST(terra_user_1, 47956924, 1); 
    await test_deposit_UST(terra_user_2, 47956924, 2); 
    await test_deposit_UST(terra_user_3, 47956924, 3); 

    await test_withdraw_UST(terra_user_1, 47956924, 1); 
    await test_withdraw_UST(terra_user_2, 47956924, 2); 
    await test_withdraw_UST(terra_user_3, 47956924, 3);     

    // Deploy the contracts
    console.log(chalk.yellow("\n Deploying Red Bank... "));
    await setupTest();


    

})();



function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}


