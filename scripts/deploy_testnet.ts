import 'dotenv/config.js'
import {
  deployContract,
  executeContract,
  instantiateContract,
  queryContract,
  recover,
  setTimeoutDuration,
  uploadContract,
} from "./helpers/helpers.js"
import { LCDClient, LocalTerra, Wallet, MnemonicKey } from "@terra-money/terra.js"
// import { testnet, bombay, local } from "./deploy_configs.js"
import { join } from "path"
import { update_LP_Staking_config, stake_LP_Tokens, claim_LPstaking_rewards, unstake_LP_Tokens
        , query_LPStaking_config, query_LPStaking_state, query_LPStaking_stakerInfo,query_LPStaking_timestamp } from "./helpers/lpStaking_helpers.js"
import { parse } from 'dotenv/types'

// consts

const MARS_ARTIFACTS_PATH = "../artifacts"

// main

async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-10'})
  let wallet = recover(terra, process.env.TEST_MAIN!)

  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)

  let mars_token_address = null;
  let address_provider = null;

  // let block = await terra.tendermint.blockInfo()
  // let dt = new Date(block.block.header.time.toString())
  console.log(Date.now())
  // console.log("Current Block Height = " + block.block.header.height.toString()  )

    
  /*************************************** Deploy CW20 (LP Token) Contract *****************************************/
//   console.log("Deploying LP Token...")

//   let lp_token_init_msg = {
//     "name": "LP-MARS",
//     "symbol": "LP-MARS",
//     "decimals": 6,
//     "initial_balances": [ {"address":wallet.key.accAddress, "amount":"100000000000000"}], // 
//     "mint": { "minter":wallet.key.accAddress, "cap":"100000000000000"}
// }

  // const lpTokenContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'cw20_token.wasm'),lp_token_init_msg)
  const lpTokenContractAddress = "terra1aztaffp5cppde68vugxay6l40rzueewau9wwld"
  // console.log("LP Token Contract Address: " + lpTokenContractAddress)


  // /*************************************** Deploy LP Staking Contract *****************************************/
  console.log("Deploying LP Staking Contract...")

  let init_timestamp = 10000 + parseInt((Date.now()/1000).toFixed(0));
  let till_timestamp = init_timestamp + 1000000;

  let lp_staking_init_msg = {
    "owner": wallet.key.accAddress,
    // "address_provider": address_provider,
    "staking_token": lpTokenContractAddress,
    "init_timestamp": init_timestamp,
    "till_timestamp": till_timestamp, 
    // "cycle_rewards": null,
    // "cycle_duration": null,
    // "reward_increase": null,
  }

  console.log(lp_staking_init_msg);
  const stakingContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'mars_lp_staking.wasm'), lp_staking_init_msg)
  // const stakingContractAddress = "terra13mtr4e5utcwy3zcsqmrceu4vde6w65nkj4qvhr"
  console.log("LP STAKING Contract Address: " + stakingContractAddress)

  // TRANSFER MARS TOKENS TO THE STAKING CONTRACT :: TO BE DISTRIBUTED AS REWARDS
  // let mars_rewards = 5000000000000;
  // await transfer_Tokens(terra, wallet, marsTokenContractAddress, stakingContractAddress, mars_rewards);

  // CONFIG :: QUERY
  // let config = await query_Contract(terra, stakingContractAddress, {"config":{}})
  // console.log(config)

  // STATE :: QUERY
  // let state = await query_Contract(terra, stakingContractAddress, { "state":{} } )
  // console.log(state)

  // BOND LP TOKENS
  // let staked_amount = 100000000;
  // await stake_LP_Tokens(terra, wallet, stakingContractAddress, lpTokenContractAddress, staked_amount )

  // STAKER INFO :: QUERY
  // let staker_info = await query_Contract(terra, stakingContractAddress, {"staker_info":{ "staker": wallet.key.accAddress }})
  // console.log(staker_info)


  // CLAIM REWARDS
  // await claim_LPstaking_rewards(terra, wallet, stakingContractAddress, marsTokenContractAddress)


  // UN-BOND LP TOKENS
  // let unstaked_amount = 1;
  // await unstake_LP_Tokens(terra, wallet, stakingContractAddress, lpTokenContractAddress, unstaked_amount )
  
  // STATE :: QUERY
  // state = await query_Contract(terra, stakingContractAddress, { "state":{} } )
  // console.log(state)

  // STAKER INFO :: QUERY
  // staker_info = await query_Contract(terra, stakingContractAddress, {"staker_info":{ "staker": wallet.key.accAddress }})
  // console.log(staker_info)


}

main().catch(console.log)
