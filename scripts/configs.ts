export const bombay_testnet: Config = { 
    lpStaking_InitMsg: {
        "config" : { 
            "owner": null,
            "address_provider": null,
            "staking_token": null,
            "init_timestamp": null,
            "till_timestamp": null, 
            "cycle_rewards": "1000000000",
            "cycle_duration": 1000000,
            "reward_increase": "0.02"
        }
    },
    lockdrop_InitMsg: {
        "config" : { 
            "owner": null,
            "address_provider": null,
            "ma_ust_token": null,
            "init_timestamp": null,
            "min_duration": 30, 
            "max_duration": 270,
            "denom": "uusd",
            "multiplier": "0.02",
            "lockdrop_incentives": "5000000000000"
        }
    },
    airdrop_InitMsg: {
        "config" : { 
            "owner": null,
            "mars_token_address": null,
            "terra_merkle_roots": [],
            "evm_merkle_roots": [],
            "till_timestamp": null, 
        } 
    }
}






interface LPStakingInitMsg {
    config : { 
        owner?: string
        address_provider?: string
        staking_token?: string
        init_timestamp?: number
        till_timestamp?: number 
        cycle_rewards: string
        cycle_duration: number
        reward_increase: string
    }
}


interface LockdropInitMsg {
    config : { 
        owner?: string
        address_provider?: string
        ma_ust_token?: string
        init_timestamp?: number
        min_duration?: number 
        max_duration: number
        denom: string
        multiplier: string
        lockdrop_incentives: string
    }
}


interface AirdropInitMsg {
    config : { 
        owner?: string
        mars_token_address?: string
        terra_merkle_roots?: []
        evm_merkle_roots?: []
        till_timestamp: number 
    }
}


interface Config {
    lpStaking_InitMsg: LPStakingInitMsg
    lockdrop_InitMsg: LockdropInitMsg
    airdrop_InitMsg: AirdropInitMsg
}
