use bytes::Bytes;
use ethereum_rust_rlp::error::RLPDecodeError;
use serde::{Deserialize, Serialize};

use ethereum_rust_core::{
    serde_utils,
    types::{
        compute_transactions_root, compute_withdrawals_root, BlobsBundle, Block, BlockBody,
        BlockHash, BlockHeader, Transaction, Withdrawal, DEFAULT_OMMERS_HASH,
    },
    Address, Bloom, H256, U256,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV3 {
    parent_hash: H256,
    fee_recipient: Address,
    state_root: H256,
    receipts_root: H256,
    logs_bloom: Bloom,
    prev_randao: H256,
    #[serde(with = "serde_utils::u64::hex_str")]
    block_number: u64,
    #[serde(with = "serde_utils::u64::hex_str")]
    gas_limit: u64,
    #[serde(with = "serde_utils::u64::hex_str")]
    gas_used: u64,
    #[serde(with = "serde_utils::u64::hex_str")]
    timestamp: u64,
    #[serde(with = "serde_utils::bytes")]
    extra_data: Bytes,
    #[serde(with = "serde_utils::u64::hex_str")]
    base_fee_per_gas: u64,
    pub block_hash: H256,
    transactions: Vec<EncodedTransaction>,
    withdrawals: Vec<Withdrawal>,
    #[serde(with = "serde_utils::u64::hex_str")]
    blob_gas_used: u64,
    #[serde(with = "serde_utils::u64::hex_str")]
    excess_blob_gas: u64,
}

#[derive(Clone, Debug)]
pub struct EncodedTransaction(pub Bytes);

impl<'de> Deserialize<'de> for EncodedTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(EncodedTransaction(serde_utils::bytes::deserialize(
            deserializer,
        )?))
    }
}

impl Serialize for EncodedTransaction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde_utils::bytes::serialize(&self.0, serializer)
    }
}

impl EncodedTransaction {
    /// Based on [EIP-2718]
    /// Transactions can be encoded in the following formats:
    /// A) `TransactionType || Transaction` (Where Transaction type is an 8-bit number between 0 and 0x7f, and Transaction is an rlp encoded transaction of type TransactionType)
    /// B) `LegacyTransaction` (An rlp encoded LegacyTransaction)
    fn decode(&self) -> Result<Transaction, RLPDecodeError> {
        Transaction::decode_canonical(self.0.as_ref())
    }

    fn encode(tx: &Transaction) -> Self {
        Self(Bytes::from(tx.encode_canonical_to_vec()))
    }
}

impl ExecutionPayloadV3 {
    /// Converts an `ExecutionPayloadV3` into a block (aka a BlockHeader and BlockBody)
    /// using the parentBeaconBlockRoot received along with the payload in the rpc call `engine_newPayloadV3`
    pub fn into_block(self, parent_beacon_block_root: H256) -> Result<Block, RLPDecodeError> {
        let body = BlockBody {
            transactions: self
                .transactions
                .iter()
                .map(|encoded_tx| encoded_tx.decode())
                .collect::<Result<Vec<_>, RLPDecodeError>>()?,
            ommers: vec![],
            withdrawals: Some(self.withdrawals),
        };
        Ok(Block {
            header: BlockHeader {
                parent_hash: self.parent_hash,
                ommers_hash: *DEFAULT_OMMERS_HASH,
                coinbase: self.fee_recipient,
                state_root: self.state_root,
                transactions_root: compute_transactions_root(&body.transactions),
                receipts_root: self.receipts_root,
                logs_bloom: self.logs_bloom,
                difficulty: 0.into(),
                number: self.block_number,
                gas_limit: self.gas_limit,
                gas_used: self.gas_used,
                timestamp: self.timestamp,
                extra_data: self.extra_data,
                prev_randao: self.prev_randao,
                nonce: 0,
                base_fee_per_gas: Some(self.base_fee_per_gas),
                withdrawals_root: Some(compute_withdrawals_root(
                    &body.withdrawals.clone().unwrap_or_default(),
                )),
                blob_gas_used: Some(self.blob_gas_used),
                excess_blob_gas: Some(self.excess_blob_gas),
                parent_beacon_block_root: Some(parent_beacon_block_root),
            },
            body,
        })
    }

    pub fn from_block(block: Block) -> Self {
        Self {
            parent_hash: block.header.parent_hash,
            fee_recipient: block.header.coinbase,
            state_root: block.header.state_root,
            receipts_root: block.header.receipts_root,
            logs_bloom: block.header.logs_bloom,
            prev_randao: block.header.prev_randao,
            block_number: block.header.number,
            gas_limit: block.header.gas_limit,
            gas_used: block.header.gas_used,
            timestamp: block.header.timestamp,
            extra_data: block.header.extra_data.clone(),
            base_fee_per_gas: block.header.base_fee_per_gas.unwrap_or_default(),
            block_hash: block.header.compute_block_hash(),
            transactions: block
                .body
                .transactions
                .iter()
                .map(EncodedTransaction::encode)
                .collect(),
            withdrawals: block.body.withdrawals.unwrap_or_default(),
            blob_gas_used: block.header.blob_gas_used.unwrap_or_default(),
            excess_blob_gas: block.header.excess_blob_gas.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStatus {
    pub status: PayloadValidationStatus,
    pub latest_valid_hash: Option<H256>,
    pub validation_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PayloadValidationStatus {
    Valid,
    Invalid,
    Syncing,
    Accepted,
}

impl PayloadStatus {
    // Convenience methods to create payload status

    /// Creates a PayloadStatus with invalid status and error message
    pub fn invalid_with_err(error: &str) -> Self {
        PayloadStatus {
            status: PayloadValidationStatus::Invalid,
            latest_valid_hash: None,
            validation_error: Some(error.to_string()),
        }
    }

    /// Creates a PayloadStatus with invalid status and latest valid hash
    pub fn invalid_with_hash(hash: BlockHash) -> Self {
        PayloadStatus {
            status: PayloadValidationStatus::Invalid,
            latest_valid_hash: Some(hash),
            validation_error: None,
        }
    }

    /// Creates a PayloadStatus with syncing status and no other info
    pub fn syncing() -> Self {
        PayloadStatus {
            status: PayloadValidationStatus::Syncing,
            latest_valid_hash: None,
            validation_error: None,
        }
    }

    /// Creates a PayloadStatus with valid status and latest valid hash
    pub fn valid_with_hash(hash: BlockHash) -> Self {
        PayloadStatus {
            status: PayloadValidationStatus::Valid,
            latest_valid_hash: Some(hash),
            validation_error: None,
        }
    }
    /// Creates a PayloadStatus with valid status and latest valid hash
    pub fn valid() -> Self {
        PayloadStatus {
            status: PayloadValidationStatus::Valid,
            latest_valid_hash: None,
            validation_error: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadResponse {
    pub execution_payload: ExecutionPayloadV3, // We only handle v3 payloads
    // Total fees consumed by the block (fees paid)
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
}

// TODO: Fill BlobsBundle
impl ExecutionPayloadResponse {
    pub fn new(payload: ExecutionPayloadV3, block_value: U256) -> Self {
        Self {
            execution_payload: payload,
            block_value,
            blobs_bundle: BlobsBundle {
                commitments: vec![],
                proofs: vec![],
                blobs: vec![],
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn deserialize_payload_into_block() {
        // Payload extracted from running kurtosis, only some transactions are included to reduce it's size.
        let json = r#"{"baseFeePerGas":"0x342770c0","blobGasUsed":"0x0","blockHash":"0x4029a2342bb6d54db91457bc8e442be22b3481df8edea24cc721f9d0649f65be","blockNumber":"0x1","excessBlobGas":"0x0","extraData":"0xd883010e06846765746888676f312e32322e34856c696e7578","feeRecipient":"0x8943545177806ed17b9f23f0a21ee5948ecaa776","gasLimit":"0x17dd79d","gasUsed":"0x401640","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","parentHash":"0x2971eefd1f71f3548728cad87c16cc91b979ef035054828c59a02e49ae300a84","prevRandao":"0x2971eefd1f71f3548728cad87c16cc91b979ef035054828c59a02e49ae300a84","receiptsRoot":"0x0185e8473b81c3a504c4919249a94a94965a2f61c06367ee6ffb88cb7a3ef02b","stateRoot":"0x0eb8fd0af53174e65bb660d0904e5016425a713d8f11c767c26148b526fc05f3","timestamp":"0x66846fb2","transactions":["0xf86d80843baa0c4082f618946177843db3138ae69679a54b95cf345ed759450d870aa87bee538000808360306ba0151ccc02146b9b11adf516e6787b59acae3e76544fdcd75e77e67c6b598ce65da064c5dd5aae2fbb535830ebbdad0234975cd7ece3562013b63ea18cc0df6c97d4","0xf86d01843baa0c4082f61894687704db07e902e9a8b3754031d168d46e3d586e870aa87bee538000808360306ba0f6c479c3e9135a61d7cca17b7354ddc311cda2d8df265d0378f940bdefd62b54a077786891b0b6bcd438d8c24d00fa6628bc2f1caa554f9dec0a96daa4f40eb0d7","0xf86d02843baa0c4082f6189415e6a5a2e131dd5467fa1ff3acd104f45ee5940b870aa87bee538000808360306ca084469ec8ee41e9104cbe3ad7e7fe4225de86076dd2783749b099a4d155900305a07e64e8848c692f0fc251e78e6f3c388eb303349f3e247481366517c2a5ae2d89","0xf86d03843baa0c4082f6189480c4c7125967139acaa931ee984a9db4100e0f3b870aa87bee538000808360306ba021d2d8a35b8da03d7e0b494f71c9ed1c28a195b94c298407b81d65163a79fbdaa024a9bfcf5bbe75ba35130fa784ab88cd21c12c4e7daf3464de91bc1ed07d1bf6","0xf86d04843baa0c4082f61894d08a63244fcd28b0aec5075052cdce31ba04fead870aa87bee538000808360306ca07ee42fee5e426595056ad406aa65a3c7adb1d3d77279f56ebe2410bcf5118b2ca07b8a0e1d21578e9043a7331f60bafc71d15788d1a2d70d00b3c46e0856ff56d2","0xf86d05843baa0c4082f618940b06ef8be65fcda88f2dbae5813480f997ee8e35870aa87bee538000808360306ba0620669c8d6a781d3131bca874152bf833622af0edcd2247eab1b086875d5242ba01632353388f46946b5ce037130e92128e5837fe35d6c7de2b9e56a0f8cc1f5e6", "0x02f8ef83301824048413f157f8842daf517a830186a094000000000000000000000000000000000000000080b8807a0a600060a0553db8600060c855c77fb29ecd7661d8aefe101a0db652a728af0fded622ff55d019b545d03a7532932a60ad52604260cd5360bf60ce53609460cf53603e60d05360f560d153bc596000609e55600060c6556000601f556000609155535660556057536055605853606e60595360e7605a5360d0605b5360eb60c080a03acb03b1fc20507bc66210f7e18ff5af65038fb22c626ae488ad9513d9b6debca05d38459e9d2a221eb345b0c2761b719b313d062ff1ea3d10cf5b8762c44385a6"],"withdrawals":[]}"#;
        let payload: ExecutionPayloadV3 = serde_json::from_str(json).unwrap();
        assert!(payload.into_block(H256::zero()).is_ok());
    }
}
