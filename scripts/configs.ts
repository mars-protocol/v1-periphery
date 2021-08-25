interface Config {
    lpStaking_InitMsg: LPStakingInitMsg
    lockdrop_InitMsg: LockdropInitMsg
    airdrop_InitMsg: AirdropInitMsg
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


export const testnet: Config = { 

}