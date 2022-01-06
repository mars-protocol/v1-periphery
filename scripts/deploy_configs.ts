let mainnet_init_timestamp = 1639465200;

export const mainnet: Config = {
  lockdrop_InitMsg: {
    config: {
      owner: undefined,
      address_provider: undefined,
      ma_ust_token: undefined,
      init_timestamp: mainnet_init_timestamp,
      deposit_window: 86400 * 5,
      withdrawal_window: 86400 * 2,
      lockup_durations: [
        { duration: 6, boost: "1" },
        { duration: 12, boost: "2" },
        { duration: 18, boost: "3" },
        { duration: 24, boost: "4" },
      ],
      seconds_per_duration_unit: 86400 * 7,
      weekly_multiplier: 3,
      weekly_divider: 51,
    },
  },

  airdrop_InitMsg: {
    config: {
      owner: undefined,
      mars_token_address: "",
      merkle_roots: undefined,
      from_timestamp: mainnet_init_timestamp + 86400 * 7,
      to_timestamp: mainnet_init_timestamp + 86400 * 7 + 86400 * 90,
    },
  },

  auction_InitMsg: {
    config: {
      owner: undefined,
      mars_token_address: "",
      astro_token_address: "",
      airdrop_contract_address: "",
      lockdrop_contract_address: "",
      generator_contract: "",
      mars_vesting_duration: 86400 * 90,
      lp_tokens_vesting_duration: 86400 * 90,
      init_timestamp: mainnet_init_timestamp + 86400 * 7,
      mars_deposit_window: 86400 * 5,
      ust_deposit_window: 86400 * 5,
      withdrawal_window: 86400 * 2,
    },
  },
};

let bombay_init_timestamp = 1639465200;

export const bombay_testnet: Config = {
  lockdrop_InitMsg: {
    config: {
      owner: undefined,
      address_provider: undefined,
      ma_ust_token: undefined,
      init_timestamp: bombay_init_timestamp,
      deposit_window: 3600 * 5,
      withdrawal_window: 3600 * 2,
      lockup_durations: [
        { duration: 6, boost: "1" },
        { duration: 12, boost: "2" },
        { duration: 18, boost: "3" },
        { duration: 24, boost: "4" },
      ],
      seconds_per_duration_unit: 3600,
      weekly_multiplier: 3,
      weekly_divider: 51,
    },
  },

  auction_InitMsg: {
    config: {
      owner: undefined,
      mars_token_address: "",
      astro_token_address: "",
      airdrop_contract_address: "",
      lockdrop_contract_address: "",
      generator_contract: "",
      mars_vesting_duration: 3600 * 90,
      lp_tokens_vesting_duration: 3600 * 90,
      init_timestamp: bombay_init_timestamp + 3600 * 7,
      mars_deposit_window: 3600 * 1,
      ust_deposit_window: Number(3600 * 1.5),
      withdrawal_window: 3600 * 1,
    },
  },

  airdrop_InitMsg: {
    config: {
      owner: undefined,
      mars_token_address: "",
      merkle_roots: [],
      from_timestamp: bombay_init_timestamp + 3600 * 7,
      to_timestamp: bombay_init_timestamp + 3600 * 7 + 86400 * 30,
    },
  },
};

interface AuctionInitMsg {
  config: {
    owner?: string;
    mars_token_address: string;
    astro_token_address: string;
    airdrop_contract_address: string;
    lockdrop_contract_address: string;
    generator_contract: string;
    mars_vesting_duration: number;
    lp_tokens_vesting_duration: number;
    init_timestamp: number;
    mars_deposit_window: number;
    ust_deposit_window: number;
    withdrawal_window: number;
  };
}

interface LockdropInitMsg {
  config: {
    owner?: string;
    address_provider?: string;
    ma_ust_token?: string;
    init_timestamp: number;
    deposit_window: number;
    withdrawal_window: number;
    lockup_durations: any;
    seconds_per_duration_unit: number;
    weekly_multiplier: number;
    weekly_divider: number;
  };
}

interface AirdropInitMsg {
  config: {
    owner?: string;
    mars_token_address: string;
    merkle_roots?: string[];
    from_timestamp?: number;
    to_timestamp: number;
  };
}

export interface Config {
  auction_InitMsg: AuctionInitMsg;
  lockdrop_InitMsg: LockdropInitMsg;
  airdrop_InitMsg: AirdropInitMsg;
}
