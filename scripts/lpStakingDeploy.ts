import 'dotenv/config.js'
import {
  deployContract,
  executeContract,
  instantiateContract,
  queryContract,
  recover,
  setTimeoutDuration,
  setupRedBank,
  uploadContract,
} from "./helpers.js"
import { LCDClient, LocalTerra, Wallet, MnemonicKey } from "@terra-money/terra.js"
import { testnet, bombay, local } from "./deploy_configs.js"
import { join } from "path"
import {query_Contract, transfer_Tokens, stake_LP_Tokens, claim_LPstaking_rewards, unstake_LP_Tokens} from "./lpHelpers.js"
import { parse } from 'dotenv/types'

// consts

const MARS_ARTIFACTS_PATH = "../artifacts"

// main

async function main() {
  let terra: LCDClient | LocalTerra
  let wallet: Wallet
  let deployConfig: Config

  // const isTestnet = process.env.NETWORK === "testnet" || process.env.NETWORK === "bombay"
  let NETWORK = "bombay"

  if (NETWORK === "testnet") {
    terra = new LCDClient({
      URL: 'https://tequila-lcd.terra.dev',
      chainID: 'tequila-0004'
    })
    wallet = recover(terra, process.env.TEST_MAIN!)
    deployConfig = testnet

  } else if (NETWORK === "bombay") {
    terra = new LCDClient({
      URL: 'https://bombay-lcd.terra.dev',
      chainID: 'bombay-10'
    })
    wallet = recover(terra, process.env.TEST_MAIN!)
    deployConfig = bombay
  } else {
    terra = new LocalTerra()
    wallet = (terra as LocalTerra).wallets.test1
    setTimeoutDuration(0)
    deployConfig = local
  }

  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)

  let block = await terra.tendermint.blockInfo()
  let dt = new Date(block.block.header.time.toString())
  console.log(Date.now())
  console.log("Current Block Height = " + block.block.header.height.toString()  )


    /*************************************** Deploy CW20 (MARS Token) Contract *****************************************/
    console.log("Deploying $MARS Token...")

    let mars_token_init_msg = {
      "name": "MARS",
      "symbol": "MARS",
      "decimals": 6,
      "initial_balances": [ {"address":wallet.key.accAddress, "amount":"100000000000000"}], // 
      "mint": { "minter":wallet.key.accAddress, "cap":"100000000000000"}
  }
  
    const marsTokenContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'cw20_token.wasm'),mars_token_init_msg)
    // const marsTokenContractAddress =  "terra13dfm0y3sfym3p28rxhgahpkpnrtre8g7xak3hh"
    console.log("$MARS Token Contract Address: " + marsTokenContractAddress)

    
  /*************************************** Deploy CW20 (LP Token) Contract *****************************************/
  console.log("Deploying LP Token...")

  let lp_token_init_msg = {
    "name": "LP-MARS",
    "symbol": "LP-MARS",
    "decimals": 6,
    "initial_balances": [ {"address":wallet.key.accAddress, "amount":"100000000000000"}], // 
    "mint": { "minter":wallet.key.accAddress, "cap":"100000000000000"}
}

  const lpTokenContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'cw20_token.wasm'),lp_token_init_msg)
  // const lpTokenContractAddress = "terra1xuq2aqk3dffh5chxt5u0gpttym0mm2jmwe5vvh"
  console.log("LP Token Contract Address: " + lpTokenContractAddress)


  // /*************************************** Deploy LP Staking Contract *****************************************/
  console.log("Deploying LP Staking Contract...")

  let init_timestamp = parseInt((Date.now()/1000).toFixed(0));
  let till_timestamp = init_timestamp + 100000;

  let lp_staking_init_msg = {
    "mars_token": marsTokenContractAddress,
    "staking_token": lpTokenContractAddress,
    "init_timestamp": init_timestamp,
    "till_timestamp": till_timestamp, // 
    "cycle_rewards": "100000000",
    "cycle_duration": 1000,
    "reward_increase": ".02",
  }

  console.log(lp_staking_init_msg);
  const stakingContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'mars_lp_staking.wasm'), lp_staking_init_msg)
  // const stakingContractAddress = await instantiateContract(terra, wallet, 3970, lp_staking_init_msg)
  // const stakingContractAddress = "terra13mtr4e5utcwy3zcsqmrceu4vde6w65nkj4qvhr"
  console.log("LP STAKING Contract Address: " + stakingContractAddress)

  // TRANSFER MARS TOKENS TO THE STAKING CONTRACT :: TO BE DISTRIBUTED AS REWARDS
  let mars_rewards = 5000000000000;
  await transfer_Tokens(terra, wallet, marsTokenContractAddress, stakingContractAddress, mars_rewards);

  // CONFIG :: QUERY
  let config = await query_Contract(terra, stakingContractAddress, {"config":{}})
  console.log(config)

  // STATE :: QUERY
  let state = await query_Contract(terra, stakingContractAddress, { "state":{} } )
  console.log(state)

  // BOND LP TOKENS
  // let staked_amount = 100000000;
  // await stake_LP_Tokens(terra, wallet, stakingContractAddress, lpTokenContractAddress, staked_amount )

  // STAKER INFO :: QUERY
  let staker_info = await query_Contract(terra, stakingContractAddress, {"staker_info":{ "staker": wallet.key.accAddress }})
  console.log(staker_info)


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
