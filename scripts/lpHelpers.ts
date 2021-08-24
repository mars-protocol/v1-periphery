import {
    performTransaction,
    createTransaction,
    executeContract,
    queryContract,
    toEncodedBinary
  } from "./helpers.js"
  import { LCDClient, LocalTerra, Wallet, MnemonicKey, Int } from "@terra-money/terra.js"

// QUERIES THE CONTRACT
export async function query_Contract(terra: LCDClient, contractAddress: string, queryMsg: object) {
    return await queryContract(terra, contractAddress, queryMsg);
}

// LP STAKING :: STAKE LP TOKENS
export async function stake_LP_Tokens(terra: LCDClient, wallet:Wallet, stakingContractAddress:string ,lpTokenAddress: string, amount: number) {
    let staking_msg = {
                        "send" : {
                            "contract": stakingContractAddress,
                            "amount": amount.toString(),
                            "msg": toEncodedBinary({"bond":{}}),
                        }
                      };
    let resp = await executeContract(terra, wallet, lpTokenAddress, staking_msg );
    console.log(amount.toString() + " LP Tokens staked successfully by " + wallet.key.accAddress);
}  

// LP STAKING :: CLAIM $MARS REWARDS
export async function claim_LPstaking_rewards(terra: LCDClient, wallet:Wallet, stakingContractAddress:string, marsTokenAddress:string) {
    let mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let claim_msg = { "claim":{} };
    let resp = await executeContract(terra, wallet, stakingContractAddress, claim_msg );
    let new_mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    console.log((new_mars_balance - mars_balance).toString() + " $MARS claimed as LP Staking rewards" );
}  

// LP STAKING :: UN-STAKE LP TOKENS
export async function unstake_LP_Tokens(terra: LCDClient, wallet:Wallet, stakingContractAddress:string, marsTokenAddress:string, amount:number) {
    let mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let unstake_msg = { "unbond":{"amount":amount.toString()} };
    let resp = await executeContract(terra, wallet, stakingContractAddress, unstake_msg );
    let new_mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    console.log(" LP Tokens unstaked. " + (new_mars_balance - mars_balance).toString() + " $MARS claimed as rewards" );
}  


// TRANSFER TOKENS
export async function transfer_Tokens(terra: LCDClient, wallet:Wallet, tokenContractAddress:string ,recepient: string, amount: number) {
    let transfer_msg = { "transfer" : { "recipient": recepient, "amount": amount.toString() } };
    let resp = await executeContract(terra, wallet, tokenContractAddress, transfer_msg );
    console.log(amount.toString() + " Tokens transferred to " + recepient);
}  

