import {executeContract, queryContract} from "./helpers.js";
import {Coins, Coin,StdFee, MsgExecuteContract, LCDClient, Wallet} from "@terra-money/terra.js";
import utils from 'web3-utils';
import Web3 from 'web3';

//-----------------------------------------------------
// ------ ExecuteContract :: Function signatures ------
// - updateAirdropConfig(terra, wallet, airdropContractAdr, new_config_msg) --> UPDATE CONFIG (ADMIN PRIVILEDGES NEEDED)
// - claimAirdropForTerraUser(terra, wallet, airdropContractAdr, claim_amount, merkle_proof, root_index) -->  AIRDROP CLAIM BY TERRA USER
// - claimAirdropForEVMUser(terra, wallet, airdropContractAdr, claim_amount, merkle_proof, root_index, eth_address, signature) --> AIRDROP CLAIM BY EVM USER
// - transferMarsByAdminFromAirdropContract(terra, wallet, airdropContractAdr, recepient ,amount) --> TRANSFER MARS (ADMIN PRIVILEDGES NEEDED)
//------------------------------------------------------
//------------------------------------------------------
// ----------- Queries :: Function signatures ----------
// - airdrop_getConfig(terra, airdropContractAdr) --> Returns configuration
// - airdrop_is_claimed(terra, airdropContractAdr, address) --> Returns true if airdrop already claimed, else false
// - airdrop_verifySignature(terra, airdropContractAdr, eth_user_address, signature, msg) --> Verifies ethereum signature (true / false)
//------------------------------------------------------


// UPDATE TERRA MERKLE ROOTS : EXECUTE TX
export async function updateAirdropConfig( terra: LCDClient, wallet:Wallet, airdropContractAdr: string, new_config: any) {
    let resp = await executeContract(terra, wallet, airdropContractAdr, new_config );
    console.log("AIRDROP CONTRACT :: CONFIG SUCCESSFULLY UPDATED");
}
  

// AIRDROP CLAIM BY TERRA USER : EXECUTE TX
export async function claimAirdropForTerraUser( terra: LCDClient, wallet:Wallet, airdropContractAdr: string,  claim_amount: number, merkle_proof: any, root_index: number  ) {
    if ( merkle_proof.length > 1 ) {
      let claim_for_terra_msg = { "terra_claim": {'amount': claim_amount.toString(), 'merkle_proof': merkle_proof, "root_index": root_index }};
        let resp = await executeContract(terra, wallet, airdropContractAdr, claim_for_terra_msg );
        return resp;        
    } else {
        console.log("AIRDROP TERRA CLAIM :: INVALID MERKLE PROOF");
    }
}
  
  
// AIRDROP CLAIM BY EVM USER : EXECUTE TX
export async function claimAirdropForEVMUser( terra: LCDClient, wallet:Wallet, airdropContractAdr: string, eth_address: string, claim_amount: number, merkle_proof: any, root_index: number, signature: string, msg_hash:string ) {
    if ( merkle_proof.length > 1 ) {
        let claim_for_evm_msg = { "evm_claim": {'eth_address': eth_address.substr(2,42).toLowerCase(), 'claim_amount': claim_amount.toString(), 'merkle_proof': merkle_proof, 'root_index': root_index, "signature": signature , "msg_hash": msg_hash}};
        let resp = await executeContract(terra, wallet, airdropContractAdr, claim_for_evm_msg );
        return resp;        
    } else {
        console.log("AIRDROP EVM CLAIM :: INVALID MERKLE PROOF");
    }
}


// TRANSFER MARS TOKENS : EXECUTE TX
export async function transferMarsByAdminFromAirdropContract( terra: LCDClient, wallet:Wallet, airdropContractAdr: string, recepient: string, amount: number) {
    let transfer_mars_msg = { "transfer_mars_tokens": {'recepient': recepient, 'amount': amount }};
    let resp = await executeContract(terra, wallet, airdropContractAdr, transfer_mars_msg );
    return resp;        
}




// GET CONFIG : CONTRACT QUERY
export async function airdrop_getConfig(  terra: LCDClient, airdropContractAdr: string) {
    try {
        let res = await terra.wasm.contractQuery(airdropContractAdr, { "config": {} })
        return res;
    }
    catch {
        console.log("ERROR IN airdrop_getConfig QUERY")
    }    
}

// IS CLAIMED : CONTRACT QUERY
export async function airdrop_is_claimed(  terra: LCDClient, airdropContractAdr: string, address: string ) {
    let is_claimed_msg = { "is_claimed": {'address': address }};
    try {
        let res = await terra.wasm.contractQuery(airdropContractAdr, is_claimed_msg)
        return res;
    }
    catch {
        console.log("ERROR IN airdrop_is_claimed QUERY")
    }
    
}
  

// EVM SIGNATURE VERIFICATION : CONTRACT QUERY
export async function airdrop_verifySignature(  terra: LCDClient, airdropContractAdr: string, user_address: string, signature: string, msg: string ) {
    try {
        let verify_signature_msg = { "is_valid_signature": {'evm_address':user_address, 'evm_signature': signature, 'signed_msg_hash': msg }};
        let res = await terra.wasm.contractQuery(airdropContractAdr, verify_signature_msg)
        return res;
    }
    catch {
        console.log("ERROR IN airdrop_verifySignature QUERY")
    }        
}
  
// // GET CW20 TOKEN BALANCE
// export async function getCW20Balance(terra, contract_addr, wallet_addr) {
//     let curBalance = await queryContract(terra, contract_addr, {"balance": {"address": wallet_addr}} );
//     return curBalance['balance']
// }

// // GET NATIVE TOKEN BALANCE
// export async function getUserNativeAssetBalance(terra, native_asset, wallet_addr) {
//     let res = await terra.bank.balance(  wallet_addr );
//     let balances = JSON.parse(JSON.parse(JSON.stringify( res )));
//     for (let i=0; i<balances.length;i++) {
//         if ( balances[i].denom == native_asset ) {
//             return balances[i].amount;
//         }
//     }    
//     return 0;
// }


// function print_events(response) {
//     if (response.height > 0) {
//       let events_array = JSON.parse(response["raw_log"])[0]["events"];
//       let attributes = events_array[1]["attributes"];
//       for (let i=0; i < attributes.length; i++ ) {
//         console.log(attributes[i]);
//       }
//     }
//   }


// EVM AIRDROP : SIGN THE MESSAGE
export function get_EVM_Signature(evm_account:any, msg:string) {
    var message = utils.isHexStrict(msg) ? utils.hexToUtf8(msg) : msg;
    var ethMessage = "\x19Ethereum Signed Message:\n" + message.length + message;
    let signature =  evm_account.sign(msg);    
    var web3 = new Web3(Web3.givenProvider || 'ws://some.local-or-remote.node:8546');
    let signee = web3.eth.accounts.recover(msg, signature.signature);
    return signature;
}