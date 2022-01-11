use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
/// Contracts from mars protocol
pub enum MarsContract {
    Council,
    Incentives,
    SafetyFund,
    MarsToken,
    Oracle,
    ProtocolAdmin,
    ProtocolRewardsCollector,
    RedBank,
    Staking,
    Treasury,
    Vesting,
    XMarsToken,
}

pub mod msg {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::MarsContract;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        /// Get config
        Config {},
        /// Get a single address
        Address { contract: MarsContract },
        /// Get a list of addresses
        Addresses { contracts: Vec<MarsContract> },
    }
}
