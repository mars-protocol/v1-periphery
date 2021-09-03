import {getMerkleRootsForTerraUsers, getMerkleRootsForEVMUsers, get_Terra_MerkleProof, get_EVM_MerkleProof, get_EVM_Signature}  from "./helpers/merkle_tree_utils.js";
import {
    transferCW20Tokens,
    deployContract,
    executeContract,
    instantiateContract,
    queryContract,
    recover,
    setTimeoutDuration,
    uploadContract,
  } from "./helpers/helpers.js";
  import { bombay_testnet } from "./configs.js";
  import {updateAirdropConfig, claimAirdropForTerraUser, claimAirdropForEVMUser, transferMarsByAdminFromAirdropContract
,airdrop_getConfig, airdrop_is_claimed, airdrop_verifySignature }  from "./helpers/airdrop_helpers.js";
import Web3 from 'web3';
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"


/*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

const MARS_ARTIFACTS_PATH = "../artifacts"
const MARS_TOKEN_ADDRESS = "terra1rfuctcuyyxqz468wha5m805vt43g83tep4rm5x";
const FROM_TIMESTAMP = parseInt((Date.now()/1000).toFixed(0))
const TILL_TIMESTAMP = FROM_TIMESTAMP + (86400 * 30)

async function main() {

  let terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-10'})
  let wallet = recover(terra, process.env.TEST_MAIN!)

  console.log(`Wallet address from seed: ${wallet.key.accAddress}`)

  // MERKLE ROOTS :: TERRA USERS
  let terra_merkle_roots = await getMerkleRootsForTerraUsers();
  // MERKLE ROOTS :: EVM (BSC/ETHEREUM) USERS
  let evm_merkle_roots = await getMerkleRootsForEVMUsers();

//   // AIRDROP :: INIT MSG
  bombay_testnet.airdrop_InitMsg.config.owner = wallet.key.accAddress;
  bombay_testnet.airdrop_InitMsg.config.mars_token_address = MARS_TOKEN_ADDRESS;
  bombay_testnet.airdrop_InitMsg.config.terra_merkle_roots = terra_merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.evm_merkle_roots = evm_merkle_roots;
  bombay_testnet.airdrop_InitMsg.config.from_timestamp = FROM_TIMESTAMP;
  bombay_testnet.airdrop_InitMsg.config.till_timestamp = TILL_TIMESTAMP;
  console.log(bombay_testnet.airdrop_InitMsg.config)

  const airdrop_contract_address = await deployContract(terra, wallet, join(MARS_ARTIFACTS_PATH, 'mars_airdrop.wasm'),  bombay_testnet.airdrop_InitMsg.config)
  // const airdrop_contract_address = "terra1cjqjhvedn6fjxyu8ph3ar78qwr6q9ngsn87m6l"
  console.log('AIRDROP CONTRACT ADDRESS : ' + airdrop_contract_address )

  // TRANSFER MARS TOKENS TO THE AIRDROP CONTRACT
  let mars_rewards = 50000000000;
  await transferCW20Tokens(terra, wallet, MARS_TOKEN_ADDRESS, airdrop_contract_address, mars_rewards);
  console.log( (mars_rewards/(10**6)).toString() +  ' MARS TRANSFERRED TO THE AIRDROP CONTRACT :: ' + airdrop_contract_address )

// /*************************************** AIRDROP CONTRACT :: TESTING FUNCTION CALLS  *****************************************/
  let web3 = new Web3(Web3.givenProvider || 'ws://some.local-or-remote.node:8546');


  // GET CONFIGURATION
  // let config = await airdrop_getConfig(terra, airdrop_contract_address);
  // console.log(config);

  // CHECK IF CLAIMED
  let test_terra_address = wallet.key.accAddress
  let is_claimed = await airdrop_is_claimed(terra, airdrop_contract_address, test_terra_address );
  console.log(is_claimed);

  // VERIFY SIGNATURE VIA CONTRACT QUERY
  // let test_evm_account = web3.eth.accounts.privateKeyToAccount('89fa5355adfd0879b7dc568ac8b5d543d7609a96b0d8aa0486305403b7429c50');
  // let test_msg_to_sign = "Testing"
  // let test_signature = get_EVM_Signature(test_evm_account, test_msg_to_sign);
  // let verify_response = await airdrop_verifySignature(terra, airdrop_contract_address, test_evm_account.address, test_signature, test_msg_to_sign);
  // console.log(verify_response);


  // // AIRDROP CLAIM : GET MERKLE PROOF FOR TERRA USER --> CLAIM AIRDROP IF VALID PROOF
  // let airdrop_claim_amount = 474082154
  // let terra_user_merkle_proof = get_Terra_MerkleProof( { "address":wallet.key.accAddress, "amount":airdrop_claim_amount.toString() } );
  // console.log(terra_user_merkle_proof)
  // await claimAirdropForTerraUser(terra, wallet, airdrop_contract_address, airdrop_claim_amount, terra_user_merkle_proof["proof"], terra_user_merkle_proof["root_index"])

  // let is_claimed_ = await airdrop_is_claimed(terra, airdrop_contract_address, wallet.key.accAddress );
  // console.log(is_claimed_);


  // // AIRDROP CLAIM : GET MERKLE PROOF, SIGNATURE FOR EVM USER --> CLAIM AIRDROP IF VALID PROOF
  // const eth_user_address = ""
  // let airdrop_claim_amount_evm_user = 324473973
  // let  evm_user_merkle_proof = get_EVM_MerkleProof( { "address":eth_user_address, "amount":airdrop_claim_amount_evm_user.toString() } );
  // let msg_to_sign = eth_user_address.substr(2,42).toLowerCase()  + terra_user_address + airdrop_claim_amount_evm_user.toString();  
  // let signature =  get_EVM_Signature(msg_to_sign, msg);
  // let is_valid_sig = await verifySignature( terra, airdrop_contract_address, eth_user_address, signature, msg_to_sign );
  // await claimAirdropForEVMUser( terra, wallet, airdrop_contract_address, eth_user_address, eth_claim_amount, evm_user_merkle_proof["proof"], evm_user_merkle_proof["root_index"], eth_user_address, signature );


  // // ADMIN FUNCTION : TRANSFER MARS FROM AIRDROP CONTRACT TO RECEPIENT
  // recepient = wallet.key.accAddress
  // await transferMarsByAdminFromAirdropContract(terra, wallet, airdrop_contract_address, recepient)


  // // ADMIN FUNCTION : UPDATE AIRDROP CONFIG
  // recepient = wallet.key.accAddress
  // await transferMarsByAdminFromAirdropContract(terra, wallet, airdrop_contract_address, recepient)

}

main().catch(console.log)