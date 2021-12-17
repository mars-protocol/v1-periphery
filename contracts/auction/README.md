# LP Bootstrap via Auction Contract

The LP Bootstrap via auction contract facilitates MARS-UST Astroport pool initialization during the protocol launch.

**Phase 1 :: Bootstrapping MARS and UST Side of the LP Pool**

- Airdrop recipients and lockdrop participants can delegate part / all of their MARS rewards to the auction contract.
- Any user can deposit UST directly to the auction contract to participate in the LP bootstrap auction.
- Users can deposit
- Both UST deposited & MARS delegated (if any) balances are used to calculate user's LP token shares and additional MARS incentives that he will receive for participating in the auction.

**Phase 2 :: Post MARS-UST Pool initialization**

- MARS reward withdrawals from lockdrop & airdrop contracts are enabled during the MARS-UST Pool initializaiton.
- MARS-UST LP tokens are staked with the generator contract, with LP Staking rewards allocated equally among the users based on their % LP share
- MARS incentives claimable by the users are also vested linearly on a 10 day period
- Users MARS-UST LP shares are also vested linearly on a 90 day period

## Contract Design

### Handle Messages

| Message                                   | Description                                                                                                                                                                                                                                                                                    |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::Receive`                     | ReceiveCW20 Hook which facilitates MARS tokens delegation by lockdrop participants / airdrop recipients                                                                                                                                                                                        |
| `ExecuteMsg::UpdateConfig`                | Admin function to update any of the configuration parameters.                                                                                                                                                                                                                                  |
| `ExecuteMsg::DepositUst`                  | Facilitates UST deposits by users                                                                                                                                                                                                                                                              |
| `ExecuteMsg::WithdrawUst`                 | Facilitates UST withdrawals by users. 100% amount can be withdrawn during deposit window, which is then limited to 50% during 1st half of deposit window which then decreases linearly during 2nd half of deposit window. Only 1 withdrawal can be made by a user during the withdrawal window |
| `ExecuteMsg::AddLiquidityToAstroportPool` | Admin function which facilitates Liquidity addtion to the Astroport MARS-UST Pool. Uses CallbackMsg to update state post liquidity addition to the pool                                                                                                                                        |
| `ExecuteMsg::StakeLpTokens`               | Facilitates MARS withdrawal for airdrop recipients once claims are allowed                                                                                                                                                                                                                     |
| `ExecuteMsg::ClaimRewards`                | Facilitates MARS rewards claim (staking incentives from generator and unvested lockdrop incentives) for users. Uses CallbackMsgs                                                                                                                                                               |
| `ExecuteMsg::WithdrawLpShares`            | Facilitates withdrawal of LP shares which have been unlocked for the user. Uses CallbackMsgs                                                                                                                                                                                                   |

### Query Messages

| Message              | Description                   |
| -------------------- | ----------------------------- |
| `QueryMsg::Config`   | Returns the config info       |
| `QueryMsg::State`    | Returns state of the contract |
| `QueryMsg::UserInfo` | Returns user position details |

## Build schema and run unit-tests

```
cargo schema
```

## License

TBD
