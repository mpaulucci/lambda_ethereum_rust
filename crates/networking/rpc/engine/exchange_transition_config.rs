use ethereum_rust_core::{serde_utils, H256};
use ethereum_rust_storage::Store;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

use crate::{utils::RpcErr, RpcHandler};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeTransitionConfigPayload {
    #[serde(with = "serde_utils::u128::hex_str")]
    terminal_total_difficulty: u128,
    terminal_block_hash: H256,
    #[serde(with = "serde_utils::u64::hex_str")]
    terminal_block_number: u64,
}

#[derive(Debug)]
pub struct ExchangeTransitionConfigV1Req {
    payload: ExchangeTransitionConfigPayload,
}

impl std::fmt::Display for ExchangeTransitionConfigV1Req {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ExchangeTransitionConfigV1Req {{ terminal_total_difficulty: {}, terminal_block_hash: {:?}, terminal_block_number: {} }}",
            self.payload.terminal_total_difficulty,
            self.payload.terminal_block_hash,
            self.payload.terminal_block_number
        )
    }
}

impl RpcHandler for ExchangeTransitionConfigV1Req {
    fn parse(params: &Option<Vec<Value>>) -> Result<ExchangeTransitionConfigV1Req, RpcErr> {
        let params = params.as_ref().ok_or(RpcErr::BadParams)?;
        if params.len() != 1 {
            return Err(RpcErr::BadParams);
        };
        let payload: ExchangeTransitionConfigPayload = serde_json::from_value(params[0].clone())?;
        Ok(ExchangeTransitionConfigV1Req { payload })
    }

    fn handle(&self, storage: Store) -> Result<Value, RpcErr> {
        info!("Received new engine request: {self}");
        let payload = &self.payload;

        let chain_config = storage.get_chain_config()?;
        let terminal_total_difficulty = chain_config.terminal_total_difficulty;

        if terminal_total_difficulty.unwrap_or_default() != payload.terminal_total_difficulty {
            warn!(
                "Invalid terminal total difficulty configured: execution {:?} consensus {}",
                terminal_total_difficulty, payload.terminal_total_difficulty
            );
        };

        let block = storage.get_block_header(payload.terminal_block_number)?;
        let terminal_block_hash = block.map_or(H256::zero(), |block| block.compute_block_hash());

        serde_json::to_value(ExchangeTransitionConfigPayload {
            terminal_block_hash,
            terminal_block_number: payload.terminal_block_number,
            terminal_total_difficulty: terminal_total_difficulty.unwrap_or_default(),
        })
        .map_err(|_| RpcErr::Internal)
    }
}
