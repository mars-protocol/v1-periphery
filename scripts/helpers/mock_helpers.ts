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


// RED BANK :: INITIALIZE UUSD POOL
export async function setupUST_Market_RedBank(terra: LocalTerra | LCDClient, wallet: Wallet, contractAddress: string) {
    // console.log("Setting up initial asset liquidity pools...");
    
    //   console.log(`Red Bank :: Initializing UST Market`);
  
      let assetType =  {  "native": {  "denom": "uusd"  }   };
      let initAssetMsg = {  "init_asset": {     "asset": assetType,
                                                "asset_params": { initial_borrow_rate: "0.2",
                                                                    max_loan_to_value: "0.75",
                                                                    reserve_factor: "0.2",
                                                                    maintenance_margin: "0.85",
                                                                    liquidation_bonus: "0.1",
                                                                    interest_rate_strategy: {
                                                                    "dynamic": {
                                                                        min_borrow_rate: "0.0",
                                                                        max_borrow_rate: "1.0",
                                                                        kp_1: "0.04",
                                                                        optimal_utilization_rate: "0.9",
                                                                        kp_augmentation_threshold: "0.15",
                                                                        kp_2: "0.07"
                                                                    }
                                                                    }
                                                                },
                                                },
                            };
  
      await executeContract(terra, wallet, contractAddress, initAssetMsg);
    //   console.log(`Red Bank :: Initialized UST Market`);
  }


