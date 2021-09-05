use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use lockdrop::msg::{ConfigResponse, UserInfoResponse, LockUpInfoResponse, ExecuteMsg, CallbackMsg, InstantiateMsg, UpdateConfigMsg, QueryMsg};
use lockdrop::state::{Config, State, UserInfo, LockupInfo };

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(UpdateConfigMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(CallbackMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(UserInfoResponse), &out_dir);
    export_schema(&schema_for!(LockUpInfoResponse), &out_dir);

    export_schema(&schema_for!(Config), &out_dir);
    export_schema(&schema_for!(State), &out_dir);
    export_schema(&schema_for!(UserInfo), &out_dir);
    export_schema(&schema_for!(LockupInfo), &out_dir);
}
