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




