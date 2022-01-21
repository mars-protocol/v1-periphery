# ASTROPORT Launch : Deployment Guide

- <h2> Mars Lockdrop + LBA Launch Guide </h2>
  <br>

- **Set environment variables**

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

- **Run deployment scripts: inside `scripts` folder**

```bash
cd scripts

node --experimental-json-modules --loader ts-node/esm testnet_deploy_periphery_contracts.ts
```

- **Create airdrop JSON for testing**

```bash
cd scripts

node --experimental-json-modules --loader ts-node/esm create_airdrop_json.ts
```

- **Upload airdrop data to Mongo Atlas DB**

1. Install mongo tools https://docs.mongodb.com/database-tools/installation/installation/
2. Use mongoimport to bulk import [list_of_users_eligible_for_airdrop.json] file generated in previous step https://docs.mongodb.com/database-tools/mongoimport/
    - change --username=[username] and --password=[password] to a valid user with write privilages of Mongo Atlas cluster (do not commit credentials into source control)
    - change --collection=usersTestnet for uploading to testnet collection or --collection=users for uploading to mainnet collection
    - --drop argument is used for if you want to drop the existing collection and start again, if you want to append to existing collection remove this argument

```bash
cd scripts

mongoimport --uri=mongodb+srv://marscluster0.bvpgl.mongodb.net/airdrop --collection=users --file=list_of_users_eligible_for_airdrop.json --numInsertionWorkers 4 --username=[username] --password=[password] --jsonArray --drop
```