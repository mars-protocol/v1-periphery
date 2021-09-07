import {executeContract,  queryContract, toEncodedBinary} from "./helpers.js"
import { LocalTerra, LCDClient, Wallet } from "@terra-money/terra.js"

//-----------------------------------------------------
// ------ ExecuteContract :: Function signatures ------
// - staking_IncreasePosition(terra, wallet, stakingContractAddress, lpTokenAddress, amount) --> STAKE LP TOKENS
// - staking_DecreasePosition(terra, wallet, stakingContractAddress, marsTokenAddress, amount) --> UN-STAKE LP TOKENS
// - staking_ClaimRewards(terra, wallet, stakingContractAddress, marsTokenAddress) --> CLAIM $MARS REWARDS
// - staking_UpdateConfig(terra, wallet, stakingContractAddress, new_config_msg) --> UPDATE CONFIG
//------------------------------------------------------
//------------------------------------------------------
// ----------- Queries :: Function signatures ----------
// - staking_getConfig(terra, stakingContractAddress) --> Returns configuration
// - staking_getState(terra, stakingContractAddress, timestamp) --> Returns contract's global state
// - staking_getPositionInfo(terra, stakingContractAddress, stakerAddress, timestamp) --> Returns user's position info
// - staking_getTimestamp(terra, stakingContractAddress) --> Returns timestamp
//------------------------------------------------------


// LP STAKING :: STAKE LP TOKENS
export async function staking_IncreasePosition( terra: LocalTerra | LCDClient, wallet:Wallet, stakingContractAddress:string ,lpTokenAddress: string, amount: number) {
    let staking_msg = {
                        "send" : {
                            "contract": stakingContractAddress,
                            "amount": amount.toString(),
                            "msg": toEncodedBinary({"bond":{}}),
                        }
                      };
    let resp = await executeContract(terra, wallet, lpTokenAddress, staking_msg );
    // console.log( (amount / 1e6).toString() + " LP Tokens staked successfully by " + wallet.key.accAddress);
}  

// LP STAKING :: UN-STAKE LP TOKENS
export async function staking_DecreasePosition( terra: LocalTerra | LCDClient, wallet:Wallet, stakingContractAddress:string, marsTokenAddress:string, amount:number, withdraw_rewards: boolean ) {
    let mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let unstake_msg = { "unbond":{"amount":amount.toString(), "withdraw_pending_reward":withdraw_rewards  } };  // , 
    let resp = await executeContract(terra, wallet, stakingContractAddress, unstake_msg );
    let new_mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let rewards_claimed = (new_mars_balance["balance"] - mars_balance["balance"])/10**6 ;
    console.log(" LP Tokens unstaked. " + rewards_claimed.toString() + " $MARS (scale = 1e6) claimed as rewards" );
}  


// LP STAKING :: CLAIM $MARS REWARDS
export async function staking_ClaimRewards( terra: LocalTerra | LCDClient, wallet:Wallet, stakingContractAddress:string, marsTokenAddress:string) {
    // let mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    let claim_msg = { "claim":{} };
    let resp = await executeContract(terra, wallet, stakingContractAddress, claim_msg );
    // let new_mars_balance = await queryContract(terra, marsTokenAddress, {"balance": {"address": wallet.key.accAddress}} );
    // let rewards_claimed = (new_mars_balance["balance"] - mars_balance["balance"])/10**6 ;
    // console.log(rewards_claimed.toString() + " $MARS (scale = 1e6) claimed as LP Staking rewards" );
}  


// UPDATE CONFIGURATION
export async function staking_UpdateConfig(  terra: LocalTerra | LCDClient,  wallet:Wallet, stakingContractAddress:string, new_config_msg: any) {
    // console.log(new_config_msg)
    let resp = await executeContract(terra, wallet, stakingContractAddress, new_config_msg );
    // console.log(" LP STAKING CONTRACT : Configuration successfully updated");
}  

// Returns configuration
export async function staking_getConfig( terra: LocalTerra | LCDClient, stakingContractAddress:string) {
    try {
        return await queryContract(terra, stakingContractAddress, {"config":{}});
    } catch {
        console.log("LP Staking :: query config error");
    }
}

// Returns contract's global state
export async function staking_getState( terra: LocalTerra | LCDClient, stakingContractAddress:string, timestamp: number) {
    try {
        if (timestamp > 0) {
            let query_msg = {"state":{"timestamp":timestamp}};
            return await queryContract(terra, stakingContractAddress, query_msg);
        }
        else {
            let query_msg = {"state":{}};
            return await queryContract(terra, stakingContractAddress, query_msg);
        }
    }
    catch {
        console.log("LP Staking :: query global state error");
    }        
}

// Returns user's position info
export async function staking_getPositionInfo( terra: LocalTerra | LCDClient, stakingContractAddress:string, stakerAddress: string, timestamp: number) {
    try {
        if (timestamp > 0) {
            let query_msg = {"staker_info": {"staker":stakerAddress, "timestamp":timestamp} } ;
            return await queryContract(terra, stakingContractAddress, query_msg );
        }
        else {
            let query_msg = {"staker_info": {"staker":stakerAddress} } ;
            return await queryContract(terra, stakingContractAddress, query_msg );
        }
    }
     catch {
        console.log("LP Staking :: query staker state error");
    }      
}

// Returns timestamp
export async function staking_getTimestamp( terra: LocalTerra | LCDClient, stakingContractAddress:string) {
    return await queryContract(terra, stakingContractAddress, {"timestamp":{}});
}


