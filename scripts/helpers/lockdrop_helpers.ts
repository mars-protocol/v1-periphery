import {executeContract,  queryContract, toEncodedBinary} from "./helpers.js"
import { Coins, Coin,StdFee, LCDClient, Wallet } from "@terra-money/terra.js"

//-----------------------------------------------------
// ------ ExecuteContract :: Function signatures ------
// - update_Lockdrop_config(terra, wallet, lockdropContractAddress, new_config_msg) --> UPDATE CONFIG
// - deposit_UST_Lockdrop(terra, wallet, lockdropContractAddress, amount, duration) --> DEPOSIT UST
// - withdraw_UST_Lockdrop(terra, wallet, lockdropContractAddress, amount, duration) --> WITHDRAW UST
// - claim_rewards_lockdrop(terra, wallet, lockdropContractAddress) --> CLAIM ACCURED REWARDS
// - unlock_deposit(terra, wallet, lockdropContractAddress, duration) --> UNLOCK DEPOSITED UST
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
export async function update_Lockdrop_config( terra: LCDClient,  wallet:Wallet, lockdropContractAddress:string, new_config_msg: any) {
    let resp = await executeContract(terra, wallet, lockdropContractAddress, new_config_msg );
    console.log(" LOCKDROP CONTRACT : Configuration successfully updated");
}  

// DEPOSIT UST FOR `duration` days
export async function deposit_UST_Lockdrop( terra: LCDClient,  wallet:Wallet, lockdropContractAddress:string, amount: number, duration: number) {
    let deposit_msg = { "deposit_ust": {"duration":duration} };
    let resp = await executeContract(terra, wallet, lockdropContractAddress, deposit_msg, new Coins([new Coin('uusd',amount.toString())]) );
    console.log(" LOCKDROP CONTRACT : " + (amount/1e6).toString() + " UST DEPOSITED");
}  

// WITHDRAW UST FROM the `duration` days lockup position
export async function withdraw_UST_Lockdrop( terra: LCDClient,  wallet:Wallet, lockdropContractAddress:string, amount: number, duration: number) {
    let withdraw_msg = { "withdraw_ust": {"duration":duration, "amount":amount.toString() } };
    let resp = await executeContract(terra, wallet, lockdropContractAddress, withdraw_msg );
    console.log(" LOCKDROP CONTRACT :  " + (amount/1e6).toString() + " UST WITHDRAWN");
}  

// CLAIM REWARDS
export async function claim_rewards_lockdrop( terra: LCDClient,  wallet:Wallet, lockdropContractAddress:string) {
    let claim_msg = {"claim_rewards":{}};
    let resp = await executeContract(terra, wallet, lockdropContractAddress, claim_msg );
    console.log(" LOCKDROP CONTRACT : REWARDS CLAIMED");
}  

// UNLOCK `duration` days lockup position
export async function unlock_deposit( terra: LCDClient,  wallet:Wallet, lockdropContractAddress:string, duration: number) {
    let unlock_msg = {"unlock":{"duration":duration}};
    let resp = await executeContract(terra, wallet, lockdropContractAddress, unlock_msg );
    console.log(" LOCKDROP CONTRACT : LOCKUP UNLOCKED");
}  





// Returns configuration
export async function query_lockdrop_config(terra: LCDClient, lockdropContractAddress:string) {
    return await queryContract(terra, lockdropContractAddress, {"config":{}});
}

// Returns contract's global state
export async function query_lockdrop_state(terra: LCDClient, lockdropContractAddress:string) {
    return await queryContract(terra, lockdropContractAddress, {"state":{}});
}

// Returns user info
export async function query_lockdrop_userInfo(terra: LCDClient, lockdropContractAddress:string, userAddress:string) {
    return await queryContract(terra, lockdropContractAddress, {"user_info":{"address":userAddress}});
}

// Returns lockdrop info
export async function query_lockdrop_lockupInfo(terra: LCDClient, lockdropContractAddress:string, userAddress:string, duration: number) {
    return await queryContract(terra, lockdropContractAddress, {"lock_up_info":{"address":userAddress, "duration":duration }});
}

// Returns lockdrop info
export async function query_lockdrop_lockupInfoWithId(terra: LCDClient, lockdropContractAddress:string, lockup_id: string) {
    return await queryContract(terra, lockdropContractAddress, {"lock_up_info_with_id":{"lockup_id":lockup_id}});
}