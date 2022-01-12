pub mod msg {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        /// Deposit Terra native coins. Deposited coins must be sent in the transaction
        /// this call is made
        DepositNative {
            /// Denom used in Terra (e.g: uluna, uusd)
            denom: String,
        },
    }
}
