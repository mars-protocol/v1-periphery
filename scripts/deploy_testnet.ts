import 'dotenv/config.js'
import {
  transferCW20Tokens,
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
import { bombay_testnet } from "./configs.js"

const MARS_ARTIFACTS_PATH = "../artifacts"
const MARS_TOKEN_ADDRESS = "terra1tv2sewn80vmf2pjv9q0sgqul8sv5axg5tezf0r";
let ADDRESS_PROVIDER = "terra1ja787s45w0s5fj7h9wjst3eatka79xghd63p0v";


async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-10'})
  let wallet = recover(terra, process.env.TEST_MAIN!)

  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)


  // console.log(Date.now())

    
  /*************************************** Deploy CW20 (LP Token) Contract *****************************************/
//   console.log("Deploying LP Token...")

//   let lp_token_init_msg = {
//     "name": "LP-Token",
//     "symbol": "LP-T",
//     "decimals": 6,
//     "initial_balances": [ {"address":wallet.key.accAddress, "amount":"100000000000000"}], // 
//     "mint": { "minter":wallet.key.accAddress, "cap":"100000000000000"}
// }

//   const lpTokenContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'cw20_token.wasm'),lp_token_init_msg)
  const lpTokenContractAddress = "terra1mr4fzf3xketmyxdtxunqgacslntm8xzpawqp4w"
  console.log("LP Token Contract Address: " + lpTokenContractAddress)


  // /*************************************** Deploy LP Staking Contract *****************************************/
  // console.log("Deploying LP Staking Contract...")

  // let init_timestamp = parseInt((Date.now()/1000).toFixed(0));
  // let till_timestamp = init_timestamp + 1000000;

  // // SETTING CONFIG
  // bombay_testnet.lpStaking_InitMsg.config.owner = wallet.key.accAddress;
  // bombay_testnet.lpStaking_InitMsg.config.address_provider = ADDRESS_PROVIDER;
  // bombay_testnet.lpStaking_InitMsg.config.staking_token = lpTokenContractAddress;
  // bombay_testnet.lpStaking_InitMsg.config.init_timestamp = init_timestamp;
  // bombay_testnet.lpStaking_InitMsg.config.till_timestamp = till_timestamp;


  // console.log(bombay_testnet.lpStaking_InitMsg.config);
  // const stakingContractAddress = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'mars_lp_staking.wasm'),  bombay_testnet.lpStaking_InitMsg.config)
  const stakingContractAddress = "terra1vvkycfvkukfhnqh8fnenye9uhgld9h0y3j3443"
  console.log("LP STAKING Contract Address: " + stakingContractAddress + "\n")


  await stake_LP_Tokens(terra, wallet,stakingContractAddress, lpTokenContractAddress, 100000000);

  // await unstake_LP_Tokens(terra, wallet,stakingContractAddress, MARS_TOKEN_ADDRESS, 100000000)

  // await claim_LPstaking_rewards(terra, wallet,stakingContractAddress, MARS_TOKEN_ADDRESS);

  // TRANSFER MARS TOKENS TO THE STAKING CONTRACT :: TO BE DISTRIBUTED AS REWARDS
  // let mars_rewards = 5000000000;
  // await transferCW20Tokens(terra, wallet, MARS_TOKEN_ADDRESS, stakingContractAddress, mars_rewards);




  // let config = await query_LPStaking_config(terra, stakingContractAddress);
  // console.log(config);
  // console.log("\n");
  // let global_state = await query_LPStaking_state(terra, stakingContractAddress, 0);
  // console.log(global_state);
  // console.log("\n");
  // let position_info = await query_LPStaking_stakerInfo(terra, stakingContractAddress, wallet.key.accAddress , 0);
  // console.log(position_info);
  // console.log("\n");
  // let timestamp = await query_LPStaking_timestamp(terra, stakingContractAddress);
  // console.log(timestamp);


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
