export const mainnet: Config = {
  lockdrop_InitMsg: {
    config: {
      owner: undefined,
      address_provider: undefined,
      ma_ust_token: undefined,
      init_timestamp: 1639465200,
      deposit_window: 86400 * 5,
      withdrawal_window: 86400 * 2,
      min_duration: 2,
      max_duration: 52,
      seconds_per_week: 86400 * 7,
      weekly_multiplier: 3,
      weekly_divider: 51,
    },
  },

  airdrop_InitMsg: {
    config: {
      owner: undefined,
      mars_token_address: "",
      merkle_roots: undefined,
      from_timestamp: 1639465200 + 86400 * 7,
      to_timestamp: 1639465200 + 86400 * 7 + 86400 * 90,
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
      init_timestamp: 1639465200 + 86400 * 7,
      mars_deposit_window: 86400 * 5,
      ust_deposit_window: 86400 * 5,
      withdrawal_window: 86400 * 2,
    },
  },
};

export const bombay_testnet: Config = {
  lockdrop_InitMsg: {
    config: {
      owner: undefined,
      address_provider: undefined,
      ma_ust_token: undefined,
      init_timestamp: 0,
      deposit_window: 3600 * 1,
      withdrawal_window: 3600 * 1,
      min_duration: 2,
      max_duration: 52,
      seconds_per_week: 3600 * 7,
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
      init_timestamp: 1639465200 + 3600 * 7,
      mars_deposit_window: 3600 * 1,
      ust_deposit_window: 3600 * 1.5,
      withdrawal_window: 3600 * 1,
    },
  },

  airdrop_InitMsg: {
    config: {
      owner: undefined,
      mars_token_address: "",
      merkle_roots: [],
      from_timestamp: 0,
      to_timestamp: 0,
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
    min_duration: number;
    max_duration: number;
    seconds_per_week: number;
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
