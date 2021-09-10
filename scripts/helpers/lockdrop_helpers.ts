import {executeContract,  queryContract, toEncodedBinary} from "./helpers.js"
import { Coins, Coin,StdFee, LocalTerra, LCDClient, Wallet } from "@terra-money/terra.js"

//-----------------------------------------------------
// ------ ExecuteContract :: Function signatures ------
// - Lockdrop_update_config(terra, wallet, lockdropContractAddress, new_config_msg) --> UPDATE CONFIG
// - Lockdrop_deposit_UST(terra, wallet, lockdropContractAddress, amount, duration) --> DEPOSIT UST
// - Lockdrop_withdraw_UST(terra, wallet, lockdropContractAddress, amount, duration) --> WITHDRAW UST
// - Lockdrop_deposit_UST_in_RedBank(terra, wallet, lockdropContractAddress) --> DEPOSIT UST IN THE RED BANK
// - Lockdrop_claim_rewards(terra, wallet, lockdropContractAddress) --> CLAIM ACCURED REWARDS
// - Lockdrop_unlock_deposit(terra, wallet, lockdropContractAddress, duration) --> UNLOCK DEPOSITED UST
//------------------------------------------------------
//------------------------------------------------------
// ----------- Queries :: Function signatures ----------
// - query_lockdrop_config(terra, lockdropContractAddress) --> Returns configuration
// - query_lockdrop_state(terra, lockdropContractAddress) --> Returns contract's global state
// - query_lockdrop_userInfo(terra, lockdropContractAddress, userAddress) --> Returns user's aggregated info
// - query_lockdrop_lockupInfo(terra, lockdropContractAddress, userAddress, duration) --> Returns Lockup details (if it exists)
// - query_lockdrop_lockupInfoWithId(terra, lockdropContractAddress, lockup_id) --> Returns Lockup details (if it exists)
//------------------------------------------------------


// UPDATE CONFIGURATION
export async function Lockdrop_update_config( terra: LocalTerra | LCDClient,  wallet:Wallet, lockdropContractAddress:string, new_config_msg: any) {
    let resp = await executeContract(terra, wallet, lockdropContractAddress, new_config_msg );
    // console.log(" LOCKDROP CONTRACT : Configuration successfully updated");
}  

// DEPOSIT UST FOR `duration` days
export async function Lockdrop_deposit_UST( terra: LocalTerra | LCDClient,  wallet:Wallet, lockdropContractAddress:string, amount: number, duration: number) {
    let deposit_msg = { "deposit_ust": {"duration":duration} };
    let resp = await executeContract(terra, wallet, lockdropContractAddress, deposit_msg, new Coins([new Coin('uusd',amount.toString())]) );
    // console.log(" LOCKDROP CONTRACT : " + (amount/1e6).toString() + " UST DEPOSITED");
}  

// WITHDRAW UST FROM the `duration` days lockup position
export async function Lockdrop_withdraw_UST( terra: LocalTerra | LCDClient,  wallet:Wallet, lockdropContractAddress:string, amount: number, duration: number) {
    let withdraw_msg = { "withdraw_ust": {"duration":duration, "amount":amount.toString() } };
    let resp = await executeContract(terra, wallet, lockdropContractAddress, withdraw_msg );
    // console.log(" LOCKDROP CONTRACT :  " + (amount/1e6).toString() + " UST WITHDRAWN");
}  

// CLAIM REWARDS
export async function Lockdrop_claim_rewards( terra: LocalTerra | LCDClient,  wallet:Wallet, lockdropContractAddress:string) {
    let claim_msg = {"claim_rewards":{}};
    let resp = await executeContract(terra, wallet, lockdropContractAddress, claim_msg );
    // console.log(" LOCKDROP CONTRACT : REWARDS CLAIMED");
}  

// UNLOCK `duration` days lockup position
export async function Lockdrop_unlock_deposit( terra: LocalTerra | LCDClient,  wallet:Wallet, lockdropContractAddress:string, duration: Number) {
    let unlock_msg = {"unlock":{"duration":duration}};
    let resp = await executeContract(terra, wallet, lockdropContractAddress, unlock_msg );
    // console.log(" LOCKDROP CONTRACT : LOCKUP UNLOCKED");
}  


export async function Lockdrop_deposit_UST_in_RedBank(terra: LocalTerra | LCDClient, wallet:Wallet, lockdropContractAddress:string) {
    let resp = await executeContract(terra, wallet, lockdropContractAddress, {"deposit_ust_in_red_bank":{}} );
    // console.log(" LOCKDROP CONTRACT : UST DEPOSITED IN RED BANK");
}



// Returns configuration
export async function query_lockdrop_config(terra: LocalTerra | LCDClient, lockdropContractAddress:string) {
    return await queryContract(terra, lockdropContractAddress, {"config":{}});
}

// Returns contract's global state
export async function query_lockdrop_state(terra: LocalTerra | LCDClient, lockdropContractAddress:string) {
    return await queryContract(terra, lockdropContractAddress, {"state":{}});
}

// Returns user info
export async function query_lockdrop_userInfo(terra: LocalTerra | LCDClient, lockdropContractAddress:string, userAddress:string) {
    return await queryContract(terra, lockdropContractAddress, {"user_info":{"address":userAddress}});
}

// Returns lockdrop info
export async function query_lockdrop_lockupInfo(terra: LocalTerra | LCDClient, lockdropContractAddress:string, userAddress:string, duration: number) {
    return await queryContract(terra, lockdropContractAddress, {"lock_up_info":{"address":userAddress, "duration":duration }});
}

// Returns lockdrop info
export async function query_lockdrop_lockupInfoWithId(terra: LocalTerra | LCDClient, lockdropContractAddress:string, lockup_id: string) {
    return await queryContract(terra, lockdropContractAddress, {"lock_up_info_with_id":{"lockup_id":lockup_id}});
}