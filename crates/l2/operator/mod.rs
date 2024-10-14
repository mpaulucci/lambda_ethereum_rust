use crate::utils::{
    config::{
        engine_api::EngineApiConfig, eth::EthConfig, operator::OperatorConfig, read_env_file,
    },
    engine_client::EngineClient,
    eth_client::EthClient,
};
use errors::OperatorError;
use ethereum_rust_blockchain::constants::TX_GAS_COST;
use ethereum_rust_core::types::{Block, EIP1559Transaction, TxKind};
use ethereum_rust_rlp::encode::RLPEncode;
use ethereum_rust_rpc::types::fork_choice::{ForkChoiceState, PayloadAttributesV3};
use ethereum_rust_storage::Store;
use ethereum_types::{Address, H256};
use keccak_hash::keccak;
use libsecp256k1::SecretKey;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{error, info, warn};

pub mod l1_watcher;
pub mod proof_data_provider;

pub mod errors;

const COMMIT_FUNCTION_SELECTOR: [u8; 4] = [241, 79, 203, 200];
const VERIFY_FUNCTION_SELECTOR: [u8; 4] = [142, 118, 10, 254];
pub struct Operator {
    eth_client: EthClient,
    engine_client: EngineClient,
    block_executor_address: Address,
    l1_address: Address,
    l1_private_key: SecretKey,
    block_production_interval: Duration,
}

pub async fn start_operator(store: Store) {
    info!("Starting Operator");

    if let Err(e) = read_env_file() {
        warn!("Failed to read .env file: {e}");
    }

    let l1_watcher = tokio::spawn(l1_watcher::start_l1_watcher(store.clone()));
    let proof_data_provider = tokio::spawn(proof_data_provider::start_proof_data_provider());
    let operator = tokio::spawn(async move {
        let eth_config = EthConfig::from_env().expect("EthConfig::from_env");
        let operator_config = OperatorConfig::from_env().expect("OperatorConfig::from_env");
        let engine_config = EngineApiConfig::from_env().expect("EngineApiConfig::from_env");
        let operator = Operator::new_from_config(&operator_config, eth_config, engine_config)
            .expect("Operator::new_from_config");
        let head_block_hash = {
            let current_block_number = store
                .get_latest_block_number()
                .expect("store.get_latest_block_number")
                .expect("store.get_latest_block_number returned None");
            store
                .get_canonical_block_hash(current_block_number)
                .expect("store.get_canonical_block_hash")
                .expect("store.get_canonical_block_hash returned None")
        };
        operator
            .start(head_block_hash, store)
            .await
            .expect("Operator::start");
    });
    tokio::try_join!(l1_watcher, proof_data_provider, operator).expect("tokio::try_join");
}

impl Operator {
    pub fn new_from_config(
        operator_config: &OperatorConfig,
        eth_config: EthConfig,
        engine_config: EngineApiConfig,
    ) -> Result<Self, OperatorError> {
        Ok(Self {
            eth_client: EthClient::new(&eth_config.rpc_url),
            engine_client: EngineClient::new_from_config(engine_config)?,
            block_executor_address: operator_config.block_executor_address,
            l1_address: operator_config.l1_address,
            l1_private_key: operator_config.l1_private_key,
            block_production_interval: Duration::from_millis(operator_config.interval_ms),
        })
    }

    pub async fn start(&self, head_block_hash: H256, store: Store) -> Result<(), OperatorError> {
        let mut head_block_hash = head_block_hash;
        loop {
            head_block_hash = self.produce_block(head_block_hash).await?;

            // TODO: Check what happens with the transactions included in the payload of the failed block.
            if head_block_hash == H256::zero() {
                error!("Failed to produce block");
                continue;
            }

            let block = store
                .get_block_by_hash(head_block_hash)
                .map_err(|error| {
                    OperatorError::FailedToRetrieveBlockFromStorage(error.to_string())
                })?
                .ok_or(OperatorError::FailedToProduceBlock(
                    "Failed to get block by hash from storage".to_string(),
                ))?;

            let commitment = keccak(block.encode_to_vec());

            match self.send_commitment(commitment).await {
                Ok(commit_tx_hash) => {
                    info!(
                    "Sent commitment to block {head_block_hash:#x}, with transaction hash {commit_tx_hash:#x}"
                );
                }
                Err(error) => {
                    error!("Failed to send commitment to block {head_block_hash:#x}. Manual intervention required: {error}");
                    panic!("Failed to send commitment to block {head_block_hash:#x}. Manual intervention required: {error}");
                }
            }

            let proof = Vec::new();

            match self.send_proof(&proof).await {
                Ok(verify_tx_hash) => {
                    info!(
                    "Sent proof for block {head_block_hash}, with transaction hash {verify_tx_hash:#x}"
                );
                }
                Err(error) => {
                    error!("Failed to send commitment to block {head_block_hash:#x}. Manual intervention required: {error}");
                    panic!("Failed to send commitment to block {head_block_hash:#x}. Manual intervention required: {error}");
                }
            }

            sleep(self.block_production_interval).await;
        }
    }

    pub async fn produce_block(&self, head_block_hash: H256) -> Result<H256, OperatorError> {
        info!("Producing block");
        let fork_choice_state = ForkChoiceState {
            head_block_hash,
            safe_block_hash: head_block_hash,
            finalized_block_hash: head_block_hash,
        };
        let payload_attributes = PayloadAttributesV3 {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            ..Default::default()
        };
        let fork_choice_response = match self
            .engine_client
            .engine_forkchoice_updated_v3(fork_choice_state, payload_attributes)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                error!("Error sending forkchoiceUpdateV3: {error}");
                return Err(OperatorError::FailedToProduceBlock(format!(
                    "forkchoiceUpdateV3: {error}",
                )));
            }
        };
        let payload_id =
            fork_choice_response
                .payload_id
                .ok_or(OperatorError::FailedToProduceBlock(
                    "payload_id is None in ForkChoiceResponse".to_string(),
                ))?;
        let execution_payload_response =
            match self.engine_client.engine_get_payload_v3(payload_id).await {
                Ok(response) => response,
                Err(error) => {
                    error!("Error sending getPayloadV3: {error}");
                    return Err(OperatorError::FailedToProduceBlock(format!(
                        "getPayloadV3: {error}"
                    )));
                }
            };
        let payload_status = match self
            .engine_client
            .engine_new_payload_v3(
                execution_payload_response.execution_payload,
                Default::default(),
                Default::default(),
            )
            .await
        {
            Ok(response) => response,
            Err(error) => {
                error!("Error sending newPayloadV3: {error}");
                return Err(OperatorError::FailedToProduceBlock(format!(
                    "newPayloadV3: {error}"
                )));
            }
        };
        let produced_block_hash =
            payload_status
                .latest_valid_hash
                .ok_or(OperatorError::FailedToProduceBlock(
                    "latest_valid_hash is None in PayloadStatus".to_string(),
                ))?;
        info!("Produced block {produced_block_hash:#x}");
        Ok(produced_block_hash)
    }

    pub async fn prepare_commitment(&self, block: Block) -> H256 {
        info!("Preparing commitment");
        keccak(block.encode_to_vec())
    }

    pub async fn send_commitment(&self, commitment: H256) -> Result<H256, OperatorError> {
        info!("Sending commitment");
        let mut calldata = Vec::with_capacity(68);
        calldata.extend(COMMIT_FUNCTION_SELECTOR);
        calldata.extend(commitment.0);

        let tx = EIP1559Transaction {
            to: TxKind::Call(self.block_executor_address),
            data: calldata.into(),
            chain_id: 3151908,
            ..Default::default()
        };

        let commit_tx_hash = self.send_transaction(tx).await?;

        info!("Commitment sent: {commit_tx_hash:#?}");

        while self
            .eth_client
            .get_transaction_receipt(commit_tx_hash)
            .await?
            .is_none()
        {
            sleep(Duration::from_secs(1)).await;
        }

        Ok(commit_tx_hash)
    }

    pub async fn send_proof(&self, block_proof: &[u8]) -> Result<H256, OperatorError> {
        info!("Sending proof");
        let mut calldata = Vec::new();
        calldata.extend(VERIFY_FUNCTION_SELECTOR);
        calldata.extend(H256::from_low_u64_be(32).as_bytes());
        calldata.extend(H256::from_low_u64_be(block_proof.len() as u64).as_bytes());
        calldata.extend(block_proof);
        let leading_zeros = 32 - (calldata.len() % 32);
        calldata.extend(vec![0; leading_zeros]);

        let tx = EIP1559Transaction {
            to: TxKind::Call(self.block_executor_address),
            data: calldata.into(),
            chain_id: 3151908,
            ..Default::default()
        };

        let verify_tx_hash = self.send_transaction(tx).await?;

        info!("Proof sent: {verify_tx_hash:#?}");

        while self
            .eth_client
            .get_transaction_receipt(verify_tx_hash)
            .await?
            .is_none()
        {
            sleep(Duration::from_secs(1)).await;
        }

        Ok(verify_tx_hash)
    }

    async fn send_transaction(&self, mut tx: EIP1559Transaction) -> Result<H256, OperatorError> {
        tx.gas_limit = self
            .eth_client
            .estimate_gas(tx.clone())
            .await?
            .saturating_add(TX_GAS_COST);

        tx.max_fee_per_gas = self.eth_client.get_gas_price().await?;

        tx.nonce = self.eth_client.get_nonce(self.l1_address).await?;

        self.eth_client
            .send_eip1559_transaction(tx, self.l1_private_key)
            .await
            .map_err(OperatorError::from)
    }
}
