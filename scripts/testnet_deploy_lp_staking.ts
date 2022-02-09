import {
  deployContract,
  executeContract,
  newClient,
  readArtifact,
  writeArtifact,
} from "./helpers/helpers.js";
import { bombay_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";

const STAKING_INCENTIVES = 15000_000000; // 50 Million = 5%

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

  if (!network.mars_ust_astroport_lp_address) {
    console.log(
      `Please set the MARS-UST Astroport LP token address in the deploy config before running this script...`
    );
    return;
  }

  const INIT_TIMESTAMP = parseInt((Date.now() / 1000).toFixed(0)) + 150;

  /*************************************** DEPLOYMENT :: LP STAKING CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: LP STAKING CONTRACT  *****************************************/
  /*************************************** DEPLOYMENT :: LP STAKING CONTRACT  *****************************************/

  if (!network.lp_staking_address) {
    console.log(`${terra.config.chainID} :: Deploying LP STAKING Contract`);
    CONFIGURATION.staking_InitMsg.config.owner = wallet.key.accAddress;
    CONFIGURATION.staking_InitMsg.config.mars_token =
      network.mars_token_address;
    CONFIGURATION.staking_InitMsg.config.staking_token = network.ma_ust_token;
    CONFIGURATION.staking_InitMsg.config.cycle_rewards =
      String(STAKING_INCENTIVES);
    CONFIGURATION.staking_InitMsg.config.init_timestamp = INIT_TIMESTAMP;
    CONFIGURATION.staking_InitMsg.config.till_timestamp =
      INIT_TIMESTAMP + 365 * 86400;

    console.log(CONFIGURATION.staking_InitMsg);
    network.lp_staking_address = await deployContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "mars_lp_staking.wasm"),
      CONFIGURATION.staking_InitMsg.config,
      "MARS Protocol -::- LP Staking contract"
    );
    writeArtifact(network, terra.config.chainID);
    console.log(
      `${terra.config.chainID} :: LP Staking Contract Address : ${network.lp_staking_address} \n`
    );
  }
}

main().catch(console.log);
