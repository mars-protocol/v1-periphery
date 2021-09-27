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
// import { update_Lockdrop_config, deposit_UST_Lockdrop, withdraw_UST_Lockdrop, claim_rewards_lockdrop
//           , unlock_deposit, deposit_UST_in_RedBank, query_lockdrop_config, query_lockdrop_state,query_lockdrop_userInfo, query_lockdrop_lockupInfo, query_lockdrop_lockupInfoWithId } from "./helpers/lockdrop_helpers.js"
import { parse } from 'dotenv/types'
import { bombay_testnet } from "./configs.js"
import {addressProvider_updateConfig, addressProvider_getAddress} from "./helpers/mock_helpers.js"; 



const MARS_ARTIFACTS_PATH = "../artifacts"
const MARS_TOKEN_ADDRESS = "terra1rfuctcuyyxqz468wha5m805vt43g83tep4rm5x";

// let ADDRESS_PROVIDER = "terra1xam9sgq9zdgxmetuy6m69usl94urjqpesj8yu2";
// let MA_UST_TOKEN_ADDRESS = "terra1gucxqmygvxcly9n4qkxmndqt0g38y0zu7hywkt";



async function main() {

  const ARTIFACTS_PATH = "../artifacts"
  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-11'})
  let deployer = recover(terra, process.env.TEST_MAIN!)

  console.log(`Wallet address from seed: ${deployer.key.accAddress}`)


  // /*************************************** Deploy LOCKDROP Contract *****************************************/

  console.log("\n Deploying Lockdrop Contract...")

  let init_timestamp = parseInt((Date.now()/1000).toFixed(0)) + 100;

  // SETTING CONFIG
  bombay_testnet.lockdrop_InitMsg.config.owner = deployer.key.accAddress;
  bombay_testnet.lockdrop_InitMsg.config.init_timestamp = init_timestamp;

  const lockdropContractAddress = await deployContract(terra, deployer, join(MARS_ARTIFACTS_PATH, 'terra_mars_lockdrop.wasm'),  bombay_testnet.lockdrop_InitMsg.config)
  console.log("LOCKDROP Contract Address: " + lockdropContractAddress + "\n")

  // TRANSFER TO LOCKDROP CONTRACT
  await transferCW20Tokens(terra, deployer, MARS_TOKEN_ADDRESS, lockdropContractAddress, 50000000000 );
  














}








main().catch(console.log)


