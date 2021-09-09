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
let xmars_token_address: string;
let incentives_address: string;
let red_bank_address: string;
let lockdrop_contract_address: string;


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

    // Deploy XMARS Token
    let xmars_token_config = { "name": "X-MARS",
                            "symbol": "XMARS",
                            "decimals": 6,
                            "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
                            "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
                           }
    xmars_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  xmars_token_config )
    console.log(chalk.green(`XMARS Token deployed successfully, address : ${chalk.cyan(xmars_token_address)}`));
    
   // Deploy maUST Token
   let ma_ust_token_config = { "name": "MA_UST",
                              "symbol": "MaUST",
                              "decimals": 6,
                              "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
                              "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
    }
    xmars_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  ma_ust_token_config )
    console.log(chalk.green(`maUST Token deployed successfully, address : ${chalk.cyan(xmars_token_address)}`));


    // Deploy Address Provider (Mock)
    address_provider_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'address_provider.wasm'),  {"owner": deployer.key.accAddress } )    
    await addressProvider_updateConfig(terra, deployer, address_provider_contract_address, { "update_config" :  { "config" : { "mars_token_address": mars_token_address,
                                                                                                                              "incentives_address": incentives_address, 
                                                                                                                              "xmars_token_address": xmars_token_address,
                                                                                                                              "red_bank_address": red_bank_address
                                                                                                                            }  
                                                                                                                } 
                                                                                            }
                                      );
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
    
    lockdrop_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'staking.wasm'),  staking_config )    
    const stakingConfigResponse = await staking_getConfig(terra, lockdrop_contract_address);
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

    console.log(chalk.green(`Staking Contract deployed successfully, address : ${chalk.cyan(lockdrop_contract_address)}`));

    var contract_mars_balance_before_transfer = await getCW20Balance(terra, mars_token_address, lockdrop_contract_address);
    var deployer_mars_balance_before_transfer = await getCW20Balance(terra, mars_token_address, deployer.key.accAddress);

    await transferCW20Tokens(terra, deployer, mars_token_address, lockdrop_contract_address, 2500000 * 10**6 );

    var contract_mars_balance_after_transfer = await getCW20Balance(terra, mars_token_address, lockdrop_contract_address);
    var deployer_mars_balance_after_transfer = await getCW20Balance(terra, mars_token_address, deployer.key.accAddress);

    expect(Number(contract_mars_balance_after_transfer) - Number(contract_mars_balance_before_transfer)).to.equal(2500000 * 10**6);
    expect(Number(deployer_mars_balance_before_transfer) - Number(deployer_mars_balance_after_transfer)).to.equal(2500000 * 10**6);

    await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_1.key.accAddress, 500000 * 10**6)
    await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_2.key.accAddress, 500000 * 10**6)
    await transferCW20Tokens(terra, deployer, lp_token_address, terra_user_3.key.accAddress, 500000 * 10**6)
}
