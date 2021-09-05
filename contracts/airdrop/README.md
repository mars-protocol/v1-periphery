# Airdrop

The Airdrop contract is for airdropping MARS tokens during the intital protocol launch. 


### Selection Criteria 
Refer to the blog here to understand how the MARS airdrop for Terra, Ethereum and BSC users were calculated, 


## Contract Design

### Handle Messages

| Message                       | Description                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::ClaimByTerraUser`   |  Executes an airdrop claim for the Terra User                                                           |
| `ExecuteMsg::ClaimByEvmUser`    | Executes an airdrop claim for the EVM User                                         |
| `ExecuteMsg::TransferMarsTokens`          | Admin function. Transfers MARS tokens available with the contract to the recepient address.                                       |
| `ExecuteMsg::UpdateConfig`    | Admin function to update any of the configuration parameters.                                      |

### Query Messages

| Message              | Description                                                                        |
| -------------------- | ---------------------------------------------------------------------------------- |
| `QueryMsg::Config`   | Returns the config info                                                            |
| `QueryMsg::IsClaimed`    |Returns a boolean value indicating if the corresponding address have yet claimed the airdrop or not                                                |
| `QueryMsg::IsValidSignature` | Returns the recovered public key, corresponding evm address (lower case without `0x` prefix) and a boolean value indicating if the message was indeed signed by the provided address or not                                           |


## Get Merkle Proof
 <TB ADDED>



## Provide Evm Signatures

```
import utils from 'web3-utils';
import Web3 from 'web3';

var evm_wallet = web3.eth.accounts.privateKeyToAccount('<PRIVATE KEY>')
var msg_to_sign = <message to sign>
var signature =  evm_wallet.sign(msg)

var evm_wallet_address = evm_wallet.replace('0x', '').toLowerCase()
var signed_msg_hash = signature["messageHash"].substr(2,66)
var signature_hash = signature["signature"].substr(2,128) 

var airdrop_contract_address = "
var terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-10'})
var wallet = recover(terra, process.env.TERRA_WALLET_KEY!)
verify_signature_msg = { "is_valid_signature": {
                            'evm_address':evm_wallet_address, 
                            'evm_signature': signature_hash, 
                            'signed_msg_hash': signed_msg_hash 
                            }
                        };
var signature_response = terra.wasm.contractQuery(airdrop_contract_address, verify_signature_msg)
console.log(signature_response)
```




## License

TBD
