# Lockdrop

The lockdrop contract allows users to lock their UST for selected duration against which they are given MARS tokens pro-rata to their wighted share to the total UST deposited in the contract.

Upon expiration of the deposit window, all the locked UST is deposited in the Red Bank and users are allowed to claim their MARS allocations. 

UST deposited in the Red Bank keeps accruing XMARS tokens which are claimable by the users.

Upon expiration of the lockup, users can withdraw their deposits as interest bearing maUST tokens, redeemable against UST via the Red Bank.

Note - Users can open muliple lockup positions with different lockup periods with the lockdrop contract


## Contract Design

### Handle Messages

| Message                       | Description                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::DepositUst` | Increases user's staked LP Token balance. Only MARS-UST LP Token can be sent to this contract via the ReceiveCw20 hook                                                  |
| `ExecuteMsg::WithdrawUst`   |  Reduces user's staked position. Pending rewards are claimed and the amount by which the position is reduced are sent back to the user                                                           |
| `ExecuteMsg::ClaimRewards`    | Claim accrued MARS Rewards                                         |
| `ExecuteMsg::UpdateConfig`          | Can only be called by the admin. Can be used to update configuration parameters like % increase per cycle, cycle duration, timestamp till which staking incentives are active etc.                                     |


### Query Messages

| Message              | Description                                                                        |
| -------------------- | ---------------------------------------------------------------------------------- |
| `QueryMsg::Config`   | Returns the config info                                                            |
| `QueryMsg::State`    | Returns the contract's global state. Can be used to estimate future cycle rewards by providing the corresponding timestamp                                                |
| `QueryMsg::StakerInfo` | Returns info of a user's staked position. Can be used to estimate future rewards by providing the corresponding timestamp                                           |
| `QueryMsg::Timestamp`   | Returns the current timestamp                       |


### Query Messages

![Alt text](../../Lockdrop_msg.png?raw=true "Lockdrop Callback Msgs")


## Build schema and run unit-tests
```
cargo schema
cargo test
```


## License

TBD
