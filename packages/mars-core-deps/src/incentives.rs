pub mod msg {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        /// Query contract config
        Config {},

        /// Query info about asset incentive for a given maToken
        AssetIncentive { ma_token_address: String },

        /// Query user current unclaimed rewards
        UserUnclaimedRewards { user_address: String },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        /// Claim rewards. MARS rewards accrued by the user will be staked into xMARS before
        /// being sent.
        ClaimRewards {},
    }
}
