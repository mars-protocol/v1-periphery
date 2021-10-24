import chalk from "chalk";
import 'dotenv/config.js'
import {
  transferCW20Tokens,
  deployContract,
  recover,
  getCW20Balance
} from "./helpers/helpers.js"
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"
import {staking_UpdateConfig, staking_IncreasePosition, staking_DecreasePosition, staking_ClaimRewards,
  staking_getConfig, staking_getState, staking_getPositionInfo, staking_getTimestamp }  from "./helpers/staking_helpers.js";
import { bombay_testnet } from "./configs.js"
import {addressProvider_updateConfig, addressProvider_getAddress} from "./helpers/mock_helpers.js"; 



const MARS_ARTIFACTS_PATH = "../artifacts"
const MARS_TOKEN_ADDRESS = "terra1qs7h830ud0a4hj72yr8f7jmlppyx7z524f7gw6";


async function main() {

  const ARTIFACTS_PATH = "../artifacts"
  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-12'})
  let deployer = recover(terra, process.env.PRIVATE_KEY!)

  console.log(`Wallet address from seed: ${deployer.key.accAddress}`)

  const init_timestamp = Number(parseInt((Date.now()/1000).toFixed(0))) + 25;
  const till_timestamp = init_timestamp + (86400 * 30)



  /*************************************** Deploy CW20 (LP Token) Contract *****************************************/

    // Deploy LP Token
    // let lp_token_config = { "name": "Astro LP",
    //                         "symbol": "LPAstro",
    //                         "decimals": 6,
    //                         "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
    //                         "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
    //                        }
    // let lp_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  lp_token_config )
    // console.log(chalk.green(`LP Token deployed successfully, address : ${chalk.cyan(lp_token_address)}`));
    


  /*************************************** Deploy Address Provider (Mock) Contract *****************************************/

    // Deploy Address Provider (Mock)
    // let address_provider_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'mock_address_provider.wasm'),  {"owner": deployer.key.accAddress } )    
    // await addressProvider_updateConfig(terra, deployer, address_provider_contract_address, { "update_config" : { "config" : { "mars_token_address": MARS_TOKEN_ADDRESS }  } });
    // console.log(chalk.green(`Address provider Contract deployed successfully, address : ${chalk.cyan(address_provider_contract_address)}`));


  // /*************************************** Deploy LP Staking Contract *****************************************/

    bombay_testnet.lpStaking_InitMsg.config.owner = deployer.key.accAddress;
    bombay_testnet.lpStaking_InitMsg.config.address_provider = "terra1h3za7sapv5c6k6c2tdapwpejmqgnaqmuudhmqw" // address_provider_contract_address;
    bombay_testnet.lpStaking_InitMsg.config.staking_token = "terra1vlrzw388xyg6fju5m3vxz3m8p5vrz33lzsv07z" // lp_token_address;
    bombay_testnet.lpStaking_InitMsg.config.init_timestamp = init_timestamp;
    bombay_testnet.lpStaking_InitMsg.config.till_timestamp = till_timestamp;
    bombay_testnet.lpStaking_InitMsg.config.cycle_rewards = "100000000";
    bombay_testnet.lpStaking_InitMsg.config.cycle_duration = 86400;
    bombay_testnet.lpStaking_InitMsg.config.reward_increase = "0.02" ;


    // Deploy Staking contract
    let staking_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'terra_mars_lp_staking.wasm'),  bombay_testnet.lpStaking_InitMsg.config )    
    console.log(chalk.green(`Staking Contract deployed successfully, address : ${chalk.cyan(staking_contract_address)}`));


  // /*************************************** Transfer MARS :: to be used as incentives *****************************************/

    // let mars_rewards = 250000000
    // await transferCW20Tokens(terra, deployer, MARS_TOKEN_ADDRESS, staking_contract_address, mars_rewards );
    // console.log(chalk.green(`${mars_rewards} $MARS to be used for staking incentives successfully transferred to the LP Staking contract`));


}







main().catch(console.log)


