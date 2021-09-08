# LP Staking Contract to incentivize MARS-UST Liquidity Providers

This Contract contains the logic for LP Token staking and reward distribution. MARS tokens allocated as liquidity incentives are distributed pro-rata to stakers of the MARS-UST Astroswap pair LP token.


## Incentive Structure
 The number of MARS tokens to be distributed as incentives among the LP stakers increase by certain % (`reward_increase` parameter in `Config` struct) every cycle, where each cycle lasts for a fixed duration in terms of time elapsed. (`cycle_duration` parameter in `Config` struct)

The current cycle number and the number of MARS tokens to be distributed during the current cycle can be retrieved via the `State` query. 

## Contract Design

### Handle Messages

| Message                       | Description                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::Receive::Cw20HookMsg::Bond` | Increases user's staked LP Token balance. Only MARS-UST LP Token can be sent to this contract via the ReceiveCw20 hook                                                  |
| `ExecuteMsg::Unbond`   |  Reduces user's staked position. Pending rewards are optionally claimable (by default not claimed) during this function call
| `ExecuteMsg::Claim`    | Claim accrued MARS Rewards                                         |
| `ExecuteMsg::UpdateConfig`          | Can only be called by the admin. Can be used to update configuration parameters like % increase per cycle, init_timestamp, till_timestamp etc


### Query Messages

| Message              | Description                                                                        |
| -------------------- | ---------------------------------------------------------------------------------- |
| `QueryMsg::Config`   | Returns the config info                                                            |
| `QueryMsg::State`    | Returns the contract's global state. Can be used to estimate future cycle rewards by providing the corresponding timestamp                                                |
| `QueryMsg::StakerInfo` | Returns info of a user's staked position. Can be used to estimate future rewards by providing the corresponding timestamp                                           |
| `QueryMsg::Timestamp`   | Returns the current timestamp                       |


## License

TBD
