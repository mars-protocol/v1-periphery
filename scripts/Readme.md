# MARS Launch : Deployment Guide

- Set environment variables

  For bombay testnet -

  ```bash
  export WALLET="<mnemonic seed>"
  export LCD_CLIENT_URL="https://bombay-lcd.terra.dev"
  export CHAIN_ID="bombay-12"
  ```

  For mainnet -

  ```bash
  export WALLET="<mnemonic seed>"
  export LCD_CLIENT_URL="https://lcd.terra.dev"
  export CHAIN_ID="columbus-5"
  ```

  - Run deploy script: inside `scripts` folder,

  ```bash
    cd scripts
    node --experimental-json-modules --loader ts-node/esm testnet_deploy_periphery_contracts.js
  ```
