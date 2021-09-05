# Airdrop

The Airdrop contract is for airdropping MARS tokens during the intital protocol launch. 


### Selection Criteria 
Refer to the blog here to understand how the MARS airdrop for Terra, Ethereum and BSC users were calculated, 


## Contract Design

### Handle Messages

| Message                       | Description                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::Receive::Cw20HookMsg::Bond` | Increases user's staked LP Token balance. Only MARS-UST LP Token can be sent to this contract via the ReceiveCw20 hook                                                  |
| `ExecuteMsg::ClaimByTerraUser`   |  Executes an airdrop claim for the Terra User                                                           |
| `ExecuteMsg::ClaimByEvmUser`    | Executes an airdrop claim by the EVM User                                         |
| `ExecuteMsg::TransferMarsTokens`          | Admin function. Transfers MARS tokens available with the contract to the recepient address.                                       |
| `ExecuteMsg::UpdateConfig`    | Admin function to update any of the configuration parameters.                                      |

### Query Messages

| Message              | Description                                                                        |
| -------------------- | ---------------------------------------------------------------------------------- |
| `QueryMsg::Config`   | Returns the config info                                                            |
| `QueryMsg::IsClaimed`    |Returns a boolean value indicating if the corresponding address have yet claimed the airdrop or not                                                |
| `QueryMsg::IsValidSignature` | Returns the recovered public key, corresponding evm address (lower case without `0x` prefix) and a boolean value indicating if the message was indeed signed by the provided address or not                                           |


## License

TBD
