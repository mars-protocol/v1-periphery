import {getMerkleRootsForTerraUsers, getMerkleRootsForEVMUsers, get_Terra_MerkleProof, get_EVM_MerkleProof, get_EVM_Signature}  from "./helpers/merkle_tree_utils.js";
import {
    transferCW20Tokens,
    deployContract,
    executeContract,
    instantiateContract,
    queryContract,
    recover,
  } from "./helpers/helpers.js";
  import { bombay_testnet } from "./configs.js";
  import {updateAirdropConfig, claimAirdropForTerraUser, claimAirdropForEVMUser, transferMarsByAdminFromAirdropContract
,getAirdropConfig, isAirdropClaimed, verify_EVM_SignatureForAirdrop }  from "./helpers/airdrop_helpers.js";
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"


/*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

const MARS_ARTIFACTS_PATH = "../artifacts"
const MARS_TOKEN_ADDRESS = "terra1rfuctcuyyxqz468wha5m805vt43g83tep4rm5x";
const FROM_TIMESTAMP = parseInt((Date.now()/1000).toFixed(0))
const TILL_TIMESTAMP = FROM_TIMESTAMP + (86400 * 30)

async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-11'})
  let wallet = recover(terra, process.env.TEST_MAIN!)

  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)

  // MERKLE ROOTS :: TERRA USERS
  let terra_merkle_roots = await getMerkleRootsForTerraUsers();
 
  // MERKLE ROOTS :: EVM (BSC/ETHEREUM) USERS
  let evm_merkle_roots = await getMerkleRootsForEVMUsers();

   // AIRDROP :: INIT MSG
  bombay_testnet.airdrop_InitMsg.config.owner = wallet.key.accAddress;
  bombay_testnet.airdrop_InitMsg.config.mars_token_address = MARS_TOKEN_ADDRESS;
  bombay_testnet.airdrop_InitMsg.config.terra_merkle_roots = terra_merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.evm_merkle_roots = evm_merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.from_timestamp = FROM_TIMESTAMP;
  bombay_testnet.airdrop_InitMsg.config.till_timestamp = TILL_TIMESTAMP;


  const airdrop_contract_address = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'terra_mars_airdrop.wasm'),  bombay_testnet.airdrop_InitMsg.config)
  console.log('AIRDROP CONTRACT ADDRESS : ' + airdrop_contract_address )

  // TRANSFER MARS TOKENS TO THE AIRDROP CONTRACT
  let mars_rewards = 50000000000;
  await transferCW20Tokens(terra, wallet, MARS_TOKEN_ADDRESS, airdrop_contract_address, mars_rewards);
  console.log( (mars_rewards/(10**6)).toString() +  ' MARS TRANSFERRED TO THE AIRDROP CONTRACT :: ' + airdrop_contract_address )

}





main().catch(console.log)