use ethereum_rust_core::types::ChainConfig;
use ethereum_rust_net::types::Node;
use ethereum_rust_storage::Store;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::utils::RpcErr;

#[derive(Serialize, Debug)]
struct NodeInfo {
    enode: String,
    id: String,
    ip: String,
    name: String,
    ports: Ports,
    protocols: HashMap<String, Protocol>,
}

#[derive(Serialize, Debug)]
struct Ports {
    discovery: u16,
    listener: u16,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
enum Protocol {
    Eth(ChainConfig),
}

pub fn node_info(storage: Store, local_node: Node) -> Result<Value, RpcErr> {
    let enode_url = local_node.enode_url();
    let mut protocols = HashMap::new();

    let chain_config = storage.get_chain_config().map_err(|_| RpcErr::Internal)?;
    protocols.insert("eth".to_string(), Protocol::Eth(chain_config));

    let node_info = NodeInfo {
        enode: enode_url,
        id: hex::encode(local_node.node_id),
        name: "ethereum_rust/0.1.0/rust1.80".to_string(),
        ip: local_node.ip.to_string(),
        ports: Ports {
            discovery: local_node.udp_port,
            listener: local_node.tcp_port,
        },
        protocols,
    };
    serde_json::to_value(node_info).map_err(|_| RpcErr::Internal)
}
