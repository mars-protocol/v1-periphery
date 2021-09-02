export const bombay_testnet: Config = { 
    lpStaking_InitMsg: {
        "config" : { 
            "owner": undefined,
            "address_provider": undefined,
            "staking_token": undefined,
            "init_timestamp": undefined,
            "till_timestamp": undefined, 
            "cycle_rewards": "1000000000",
            "cycle_duration": 86400,
            "reward_increase": "0.02"
        }
    },
    lockdrop_InitMsg: {
        "config" : { 
            "owner": "",
            "address_provider": undefined,
            "ma_ust_token": undefined,
            "init_timestamp": 0,
            "deposit_window": 1800,         // 30 min
            "withdrawal_window": 900,       // 15 min
            "min_duration": 1,         
            "max_duration": 5,
            "denom": "uusd",
            "weekly_multiplier": "0.02",    // 2% 
            "lockdrop_incentives": "50000000000"
        }
    },
    airdrop_InitMsg: {
        "config" : { 
            "owner": undefined,
            "mars_token_address": undefined,
            "terra_merkle_roots": [],
            "evm_merkle_roots": [],
            "till_timestamp": undefined, 
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
        owner: string
        address_provider?: string
        ma_ust_token?: string
        init_timestamp: number
        deposit_window: number 
        withdrawal_window: number 
        min_duration: number 
        max_duration: number
        denom: string
        weekly_multiplier: string
        lockdrop_incentives: string
    }
}


interface AirdropInitMsg {
    config : { 
        owner?: string
        mars_token_address?: string
        terra_merkle_roots?: []
        evm_merkle_roots?: []
        from_timestamp?: number 
        till_timestamp?: number 
    }
}


interface Config {
    lpStaking_InitMsg: LPStakingInitMsg
    lockdrop_InitMsg: LockdropInitMsg
    airdrop_InitMsg: AirdropInitMsg
}
