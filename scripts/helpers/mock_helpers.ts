import {executeContract,  queryContract, toEncodedBinary} from "./helpers.js"
import { LocalTerra, LCDClient, Wallet } from "@terra-money/terra.js"


// UPDATE CONFIGURATION :: MOCK ADDRESS PROVIDER
export async function addressProvider_updateConfig(  terra: LocalTerra | LCDClient,  wallet:Wallet, address_provider_contract:string, new_config_msg: any) {
    let resp = await executeContract(terra, wallet, address_provider_contract, new_config_msg );
}  


// Returns user's position info
export async function addressProvider_getAddress( terra: LocalTerra | LCDClient, address_provider_contract:string, contract: string) {
    try {
            return await queryContract(terra, address_provider_contract, {"address": {"contract" : contract}  } );
    }
     catch {
        console.log("Address provider :: query addressProvider_getAddress error");
    }      
}


// SET INCENTIVES :: MOCK INCENTIVES CONTRACT
export async function incentives_set_asset_incentive(  terra: LocalTerra | LCDClient,  wallet:Wallet, incentives_contract:string, ma_token_address: string, emission_per_second: number) {
    let resp = await executeContract(terra, wallet, incentives_contract, {"set_asset_incentive": { "ma_token_address":ma_token_address , "emission_per_second":emission_per_second.toString()  }} );
}  

// UPDATE CONFIGURATION :: MOCK STAKING CONTRACT
export async function mockStaking_updateConfig(  terra: LocalTerra | LCDClient,  wallet:Wallet, mock_staking_contract:string, new_config_msg: any) {
    let resp = await executeContract(terra, wallet, mock_staking_contract, new_config_msg );
}  





