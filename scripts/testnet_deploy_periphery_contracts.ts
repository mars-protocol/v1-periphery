import "dotenv/config";
import {
  LegacyAminoMultisigPublicKey,
  MsgExecuteContract,
  SimplePublicKey,
} from "@terra-money/terra.js";
import {
  deployContract,
  executeContract,
  newClient,
  executeContractJsonForMultiSig,
  readArtifact,
  writeArtifact,
  Client,
} from "./helpers/helpers.js";
import { getMerkleRoots } from "./helpers/airdrop_helpers/merkle_tree_utils.js";
import { bombay_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";
import { writeFileSync } from "fs";

const LOCKDROP_INCENTIVES = 75_000_000_000000; // 7.5 Million = 7.5%
const AIRDROP_INCENTIVES = 25_000_000_000000; // 2.5 Million = 2.5%
const AUCTION_INCENTIVES = 10_000_000_000000; // 1.0 Million = 1%

const ARTIFACTS_PATH = "../artifacts";

async function main() {
  let CONFIGURATION: Config = bombay_testnet;

  // terra, wallet
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  // network : stores contract addresses
  let network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  if (terra.config.chainID != "bombay-12") {
    console.log("Network is not testnet. Wrong script... terminating ... ");
    return;
  }

  // MARS Token addresss should be set
  if (!network.mars_token_address) {
    console.log(
      `Please deploy the CW20-base MARS token, and then set this address in the deploy config before running this script...`
    );
    return;
  }

  // DEPOLYMENT CONFIGURATION FOR BOMBAY-12
  const LOCKDROP_INIT_TIMESTAMP =
    parseInt((Date.now() / 1000).toFixed(0)) + 150;
  const LOCKDROP_DEPOSIT_WINDOW = 3600;
  const LOCKDROP_WITHDRAWAL_WINDOW = 3600;
  const SECONDS_PER_WEEK = 3600 * 3;

  const AUCTION_MARS_DEPOSIT_WINDOW = 3600 / 2;
  const AUCTION_UST_DEPOSIT_WINDOW = 3600;
  const AUCTION_WITHDRAWAL_WINDOW = 3600;

  // LOCKDROP :: CONFIG
  CONFIGURATION.lockdrop_InitMsg.config.init_timestamp =
    LOCKDROP_INIT_TIMESTAMP;
  CONFIGURATION.lockdrop_InitMsg.config.deposit_window =
    LOCKDROP_DEPOSIT_WINDOW;
  CONFIGURATION.lockdrop_InitMsg.config.withdrawal_window =
    LOCKDROP_WITHDRAWAL_WINDOW;
  CONFIGURATION.lockdrop_InitMsg.config.seconds_per_week = SECONDS_PER_WEEK;
  // AIRDROP :: CONFIG
  CONFIGURATION.airdrop_InitMsg.config.from_timestamp =
    LOCKDROP_INIT_TIMESTAMP +
    LOCKDROP_DEPOSIT_WINDOW +
    LOCKDROP_WITHDRAWAL_WINDOW;
  CONFIGURATION.airdrop_InitMsg.config.to_timestamp =
    LOCKDROP_INIT_TIMESTAMP +
    LOCKDROP_DEPOSIT_WINDOW +
    LOCKDROP_WITHDRAWAL_WINDOW +
    86400 * 90;
  // AUCTION :: CONFIG
  CONFIGURATION.auction_InitMsg.config.init_timestamp =
    LOCKDROP_INIT_TIMESTAMP +
    LOCKDROP_DEPOSIT_WINDOW +
    LOCKDROP_WITHDRAWAL_WINDOW;
  CONFIGURATION.auction_InitMsg.config.mars_deposit_window =
    AUCTION_MARS_DEPOSIT_WINDOW;
  CONFIGURATION.auction_InitMsg.config.ust_deposit_window =
    AUCTION_UST_DEPOSIT_WINDOW;
  CONFIGURATION.auction_InitMsg.config.withdrawal_window =
    AUCTION_WITHDRAWAL_WINDOW;

  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: LOCKDROP CONTRACT  *****************************************/

  if (!network.lockdrop_address) {
    console.log(`${terra.config.chainID} :: Deploying Lockdrop Contract`);
    CONFIGURATION.lockdrop_InitMsg.config.owner = wallet.key.accAddress;
    CONFIGURATION.lockdrop_InitMsg.config.address_provider =
      network.address_provider;
    CONFIGURATION.lockdrop_InitMsg.config.ma_ust_token = network.ma_ust_token;
    console.log(CONFIGURATION.lockdrop_InitMsg);
    network.lockdrop_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_lockdrop.wasm"),
      CONFIGURATION.lockdrop_InitMsg.config,
      "MARS Protocol -::- Phase 1 -::- Lockdrop"
    );
    writeArtifact(network, terra.config.chainID);
    console.log(
      `${terra.config.chainID} :: Lockdrop Contract Address : ${network.lockdrop_address} \n`
    );
  }

  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AIRDROP CONTRACT  *****************************************/

  if (!network.airdrop_address) {
    console.log(`${terra.config.chainID} :: Deploying Airdrop Contract`);
    // Set configuration
    CONFIGURATION.airdrop_InitMsg.config.owner = wallet.key.accAddress;
    CONFIGURATION.airdrop_InitMsg.config.merkle_roots = await getMerkleRoots();
    CONFIGURATION.airdrop_InitMsg.config.mars_token_address =
      network.mars_token_address;
    // deploy airdrop contract
    console.log(CONFIGURATION.airdrop_InitMsg);
    network.airdrop_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_airdrop.wasm"),
      CONFIGURATION.airdrop_InitMsg.config,
      "MARS Protocol -::- Phase 2 -::- Airdrop"
    );
    console.log(
      `${terra.config.chainID} :: Airdrop Contract Address : ${network.airdrop_address} \n`
    );
    writeArtifact(network, terra.config.chainID);
  }

  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: AUCTION CONTRACT  *****************************************/

  if (!network.auction_address) {
    console.log(`${terra.config.chainID} :: Deploying Auction Contract`);
    // Set configuration
    CONFIGURATION.auction_InitMsg.config.owner = wallet.key.accAddress;
    CONFIGURATION.auction_InitMsg.config.mars_token_address =
      network.mars_token_address;
    CONFIGURATION.auction_InitMsg.config.astro_token_address =
      network.astro_token_address;
    CONFIGURATION.auction_InitMsg.config.airdrop_contract_address =
      network.airdrop_address;
    CONFIGURATION.auction_InitMsg.config.lockdrop_contract_address =
      network.lockdrop_address;
    CONFIGURATION.auction_InitMsg.config.generator_contract =
      network.generator_contract;
    // deploy auction contract
    console.log(CONFIGURATION.auction_InitMsg);
    network.auction_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "astroport_auction.wasm"),
      CONFIGURATION.auction_InitMsg.config,
      "MARS Protocol -::- Phase 2 -::- Auction"
    );
    console.log(
      `${terra.config.chainID} :: Auction Contract Address : ${network.auction_address} \n`
    );
    writeArtifact(network, terra.config.chainID);
  }

  //  UpdateConfig :: SET Auction Contract in Lockdrop
  //  UpdateConfig :: SET Auction Contract in Lockdrop
  if (!network.auction_set_in_lockdrop) {
    console.log(
      `${terra.config.chainID} :: Setting Auction Contract address in Lockdrop...`
    );
    let tx = await executeContract(
      terra,
      wallet,
      network.lockdrop_address,
      {
        update_config: {
          new_config: {
            owner: undefined,
            address_provider: undefined,
            ma_ust_token: undefined,
            auction_contract_address: network.auction_address,
          },
        },
      },
      []
    );
    console.log(
      `Lockdrop :: Auction Contract address set successfully set ${tx.txhash}\n`
    );
    network.auction_set_in_lockdrop = true;
    writeArtifact(network, terra.config.chainID);
  }

  // UpdateConfig :: Set Auction address in airdrop
  // UpdateConfig :: Set Auction address in airdrop
  if (!network.auction_set_in_airdrop) {
    // update Config Tx
    let out = await executeContract(
      terra,
      wallet,
      network.airdrop_address,
      {
        update_config: {
          owner: undefined,
          auction_contract_address: network.auction_address,
          merkle_roots: undefined,
          from_timestamp: undefined,
          to_timestamp: undefined,
        },
      },
      [],
      " MARS Airdrop : Set Auction address "
    );
    console.log(
      `${terra.config.chainID} :: Setting auction contract address in MARS Airdrop contract,  ${out.txhash}`
    );
    network.auction_set_in_airdrop = true;
    writeArtifact(network, terra.config.chainID);
  }

  // MARS::Send::Lockdrop::IncreaseAstroIncentives:: Transfer MARS to Lockdrop and set total incentives
  // MARS::Send::Lockdrop::IncreaseAstroIncentives:: Transfer MARS to Lockdrop and set total incentives
  if (!network.lockdrop_astro_token_transferred) {
    let transfer_msg = {
      send: {
        contract: network.lockdrop_address,
        amount: String(LOCKDROP_INCENTIVES),
        msg: Buffer.from(
          JSON.stringify({ increase_astro_incentives: {} })
        ).toString("base64"),
      },
    };
    let increase_astro_incentives = await executeContract(
      terra,
      wallet,
      network.mars_token_address,
      transfer_msg,
      [],
      "Transfer MARS to Lockdrop for Incentives"
    );
    console.log(
      `${terra.config.chainID} :: Transferring MARS Token and setting incentives in Lockdrop... ${increase_astro_incentives.txhash}`
    );
    network.lockdrop_astro_token_transferred = true;
    writeArtifact(network, terra.config.chainID);
  }

  // MARS::Send::Airdrop::IncreaseAstroIncentives:: Transfer MARS to Airdrop
  // MARS::Send::Airdrop::IncreaseAstroIncentives:: Transfer MARS to Airdrop
  if (!network.airdrop_astro_token_transferred) {
    // transfer MARS Tx
    let tx = await executeContract(
      terra,
      wallet,
      network.mars_token_address,
      {
        send: {
          contract: network.airdrop_address,
          amount: String(AIRDROP_INCENTIVES),
          msg: Buffer.from(
            JSON.stringify({ increase_astro_incentives: {} })
          ).toString("base64"),
        },
      },
      [],
      " Airdrop : Transferring MARS "
    );
    console.log(
      `${terra.config.chainID} :: Transferring MARS Token and setting incentives in Airdrop... ${tx.txhash}`
    );
    network.airdrop_astro_token_transferred = true;
    writeArtifact(network, terra.config.chainID);
  }

  // MARS::Send::Airdrop::IncreaseAstroIncentives::Transfer MARS to Auction
  // MARS::Send::Airdrop::IncreaseAstroIncentives::Transfer MARS to Auction
  if (!network.auction_astro_token_transferred) {
    // transfer MARS Tx
    let msg = {
      send: {
        contract: network.auction_address,
        amount: String(AUCTION_INCENTIVES),
        msg: Buffer.from(
          JSON.stringify({ increase_astro_incentives: {} })
        ).toString("base64"),
      },
    };
    let out = await executeContract(
      terra,
      wallet,
      network.mars_token_address,
      msg,
      [],
      " Transferring MARS Token to Auction for auction participation incentives"
    );
    console.log(
      `${terra.config.chainID} :: Transferring MARS Token and setting incentives in Auction... ${out.txhash}`
    );
    network.auction_astro_token_transferred = true;
    writeArtifact(network, terra.config.chainID);
  }
}

main().catch(console.log);
