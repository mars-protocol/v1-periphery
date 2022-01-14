# ASTROPORT Launch : Deployment Guide

- <h2> Mars Lockdrop + LBA Launch Guide </h2>
  <br>

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

- Run deployment scripts: inside `scripts` folder,

```bash
cd scripts

node --experimental-json-modules --loader ts-node/esm testnet_deploy_periphery_contracts.ts
```

- Create airdrop JSON for testing,

```bash
cd scripts

node --experimental-json-modules --loader ts-node/esm create_airdrop_json.ts
```
