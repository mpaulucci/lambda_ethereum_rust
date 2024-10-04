use super::{
    BASE_FEE_MAX_CHANGE_DENOMINATOR, BLOB_BASE_FEE_UPDATE_FRACTION, ELASTICITY_MULTIPLIER,
    GAS_LIMIT_ADJUSTMENT_FACTOR, GAS_LIMIT_MINIMUM, INITIAL_BASE_FEE, MIN_BASE_FEE_PER_BLOB_GAS,
};
use crate::{
    types::{Receipt, Transaction},
    Address, H256, U256,
};
use bytes::Bytes;
use ethereum_rust_rlp::{
    decode::RLPDecode,
    encode::RLPEncode,
    error::RLPDecodeError,
    structs::{Decoder, Encoder},
};
use ethereum_rust_trie::Trie;
use ethereum_types::Bloom;
use keccak_hash::keccak;
use serde::{Deserialize, Serialize};

use std::cmp::{max, Ordering};

pub type BlockNumber = u64;
pub type BlockHash = H256;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref DEFAULT_OMMERS_HASH: H256 = H256::from_slice(&hex::decode("1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347").unwrap()); // = Keccak256(RLP([])) as of EIP-3675
}
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub body: BlockBody,
}

impl RLPEncode for Block {
    fn encode(&self, buf: &mut dyn bytes::BufMut) {
        Encoder::new(buf)
            .encode_field(&self.header)
            .encode_field(&self.body.transactions)
            .encode_field(&self.body.ommers)
            .encode_optional_field(&self.body.withdrawals)
            .finish();
    }
}

impl RLPDecode for Block {
    fn decode_unfinished(rlp: &[u8]) -> Result<(Self, &[u8]), RLPDecodeError> {
        let decoder = Decoder::new(rlp)?;
        let (header, decoder) = decoder.decode_field("header")?;
        let (transactions, decoder) = decoder.decode_field("transactions")?;
        let (ommers, decoder) = decoder.decode_field("ommers")?;
        let (withdrawals, decoder) = decoder.decode_optional_field();
        let remaining = decoder.finish()?;
        let body = BlockBody {
            transactions,
            ommers,
            withdrawals,
        };
        let block = Block { header, body };
        Ok((block, remaining))
    }
}

/// Header part of a block on the chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockHeader {
    pub parent_hash: H256,
    #[serde(rename(serialize = "sha3Uncles"))]
    pub ommers_hash: H256, // ommer = uncle
    #[serde(rename(serialize = "miner"))]
    pub coinbase: Address,
    pub state_root: H256,
    pub transactions_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Bloom,
    #[serde(default)]
    pub difficulty: U256,
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub number: BlockNumber,
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub gas_limit: u64,
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub gas_used: u64,
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub timestamp: u64,
    #[serde(with = "crate::serde_utils::bytes")]
    pub extra_data: Bytes,
    #[serde(rename(serialize = "mixHash"))]
    pub prev_randao: H256,
    #[serde(with = "crate::serde_utils::u64::hex_str_padding")]
    pub nonce: u64,
    #[serde(with = "crate::serde_utils::u64::hex_str_opt")]
    pub base_fee_per_gas: Option<u64>,
    pub withdrawals_root: Option<H256>,
    #[serde(with = "crate::serde_utils::u64::hex_str_opt")]
    pub blob_gas_used: Option<u64>,
    #[serde(with = "crate::serde_utils::u64::hex_str_opt")]
    pub excess_blob_gas: Option<u64>,
    pub parent_beacon_block_root: Option<H256>,
}

impl RLPEncode for BlockHeader {
    fn encode(&self, buf: &mut dyn bytes::BufMut) {
        Encoder::new(buf)
            .encode_field(&self.parent_hash)
            .encode_field(&self.ommers_hash)
            .encode_field(&self.coinbase)
            .encode_field(&self.state_root)
            .encode_field(&self.transactions_root)
            .encode_field(&self.receipts_root)
            .encode_field(&self.logs_bloom)
            .encode_field(&self.difficulty)
            .encode_field(&self.number)
            .encode_field(&self.gas_limit)
            .encode_field(&self.gas_used)
            .encode_field(&self.timestamp)
            .encode_field(&self.extra_data)
            .encode_field(&self.prev_randao)
            .encode_field(&self.nonce.to_be_bytes())
            .encode_optional_field(&self.base_fee_per_gas)
            .encode_optional_field(&self.withdrawals_root)
            .encode_optional_field(&self.blob_gas_used)
            .encode_optional_field(&self.excess_blob_gas)
            .encode_optional_field(&self.parent_beacon_block_root)
            .finish();
    }
}

impl RLPDecode for BlockHeader {
    fn decode_unfinished(rlp: &[u8]) -> Result<(Self, &[u8]), RLPDecodeError> {
        let decoder = Decoder::new(rlp)?;
        let (parent_hash, decoder) = decoder.decode_field("parent_hash")?;
        let (ommers_hash, decoder) = decoder.decode_field("ommers_hash")?;
        let (coinbase, decoder) = decoder.decode_field("coinbase")?;
        let (state_root, decoder) = decoder.decode_field("state_root")?;
        let (transactions_root, decoder) = decoder.decode_field("transactions_root")?;
        let (receipts_root, decoder) = decoder.decode_field("receipts_root")?;
        let (logs_bloom, decoder) = decoder.decode_field("logs_bloom")?;
        let (difficulty, decoder) = decoder.decode_field("difficulty")?;
        let (number, decoder) = decoder.decode_field("number")?;
        let (gas_limit, decoder) = decoder.decode_field("gas_limit")?;
        let (gas_used, decoder) = decoder.decode_field("gas_used")?;
        let (timestamp, decoder) = decoder.decode_field("timestamp")?;
        let (extra_data, decoder) = decoder.decode_field("extra_data")?;
        let (prev_randao, decoder) = decoder.decode_field("prev_randao")?;
        let (nonce, decoder) = decoder.decode_field("nonce")?;
        let nonce = u64::from_be_bytes(nonce);
        let (base_fee_per_gas, decoder) = decoder.decode_optional_field();
        let (withdrawals_root, decoder) = decoder.decode_optional_field();
        let (blob_gas_used, decoder) = decoder.decode_optional_field();
        let (excess_blob_gas, decoder) = decoder.decode_optional_field();
        let (parent_beacon_block_root, decoder) = decoder.decode_optional_field();

        Ok((
            BlockHeader {
                parent_hash,
                ommers_hash,
                coinbase,
                state_root,
                transactions_root,
                receipts_root,
                logs_bloom,
                difficulty,
                number,
                gas_limit,
                gas_used,
                timestamp,
                extra_data,
                prev_randao,
                nonce,
                base_fee_per_gas,
                withdrawals_root,
                blob_gas_used,
                excess_blob_gas,
                parent_beacon_block_root,
            },
            decoder.finish()?,
        ))
    }
}

// The body of a block on the chain
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BlockBody {
    pub transactions: Vec<Transaction>,
    // TODO: ommers list is always empty, so we can remove it
    #[serde(rename(serialize = "uncles"))]
    pub ommers: Vec<BlockHeader>,
    pub withdrawals: Option<Vec<Withdrawal>>,
}

impl BlockBody {
    pub const fn empty() -> Self {
        Self {
            transactions: Vec::new(),
            ommers: Vec::new(),
            withdrawals: Some(Vec::new()),
        }
    }
}

pub fn compute_transactions_root(transactions: &[Transaction]) -> H256 {
    let iter = transactions.iter().enumerate().map(|(idx, tx)| {
        // Key: RLP(tx_index)
        // Value: tx_type || RLP(tx)  if tx_type != 0
        //                   RLP(tx)  else
        (idx.encode_to_vec(), tx.encode_canonical_to_vec())
    });
    Trie::compute_hash_from_unsorted_iter(iter)
}

pub fn compute_receipts_root(receipts: &[Receipt]) -> H256 {
    let iter = receipts
        .iter()
        .enumerate()
        .map(|(idx, receipt)| (idx.encode_to_vec(), receipt.encode_to_vec()));
    Trie::compute_hash_from_unsorted_iter(iter)
}

// See [EIP-4895](https://eips.ethereum.org/EIPS/eip-4895)
pub fn compute_withdrawals_root(withdrawals: &[Withdrawal]) -> H256 {
    let iter = withdrawals
        .iter()
        .enumerate()
        .map(|(idx, withdrawal)| (idx.encode_to_vec(), withdrawal.encode_to_vec()));
    Trie::compute_hash_from_unsorted_iter(iter)
}

impl RLPEncode for BlockBody {
    fn encode(&self, buf: &mut dyn bytes::BufMut) {
        Encoder::new(buf)
            .encode_field(&self.transactions)
            .encode_field(&self.ommers)
            .encode_optional_field(&self.withdrawals)
            .finish();
    }
}

impl RLPDecode for BlockBody {
    fn decode_unfinished(rlp: &[u8]) -> Result<(Self, &[u8]), RLPDecodeError> {
        let decoder = Decoder::new(rlp)?;
        let (transactions, decoder) = decoder.decode_field("transactions")?;
        let (ommers, decoder) = decoder.decode_field("ommers")?;
        let (withdrawals, decoder) = decoder.decode_optional_field();
        Ok((
            BlockBody {
                transactions,
                ommers,
                withdrawals,
            },
            decoder.finish()?,
        ))
    }
}

impl BlockHeader {
    pub fn compute_block_hash(&self) -> H256 {
        let mut buf = vec![];
        self.encode(&mut buf);
        keccak(buf)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdrawal {
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub index: u64,
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub validator_index: u64,
    pub address: Address,
    #[serde(with = "crate::serde_utils::u64::hex_str")]
    pub amount: u64,
}

impl RLPEncode for Withdrawal {
    fn encode(&self, buf: &mut dyn bytes::BufMut) {
        Encoder::new(buf)
            .encode_field(&self.index)
            .encode_field(&self.validator_index)
            .encode_field(&self.address)
            .encode_field(&self.amount)
            .finish();
    }
}

impl RLPDecode for Withdrawal {
    fn decode_unfinished(rlp: &[u8]) -> Result<(Self, &[u8]), RLPDecodeError> {
        let decoder = Decoder::new(rlp)?;
        let (index, decoder) = decoder.decode_field("index")?;
        let (validator_index, decoder) = decoder.decode_field("validator_index")?;
        let (address, decoder) = decoder.decode_field("address")?;
        let (amount, decoder) = decoder.decode_field("amount")?;
        Ok((
            Withdrawal {
                index,
                validator_index,
                address,
                amount,
            },
            decoder.finish()?,
        ))
    }
}

// Checks that the gas_limit fits the gas bounds set by its parent block
fn check_gas_limit(gas_limit: u64, parent_gas_limit: u64) -> bool {
    let max_adjustment_delta = parent_gas_limit / GAS_LIMIT_ADJUSTMENT_FACTOR;

    gas_limit < parent_gas_limit + max_adjustment_delta
        && gas_limit > parent_gas_limit - max_adjustment_delta
        && gas_limit >= GAS_LIMIT_MINIMUM
}

// Calculates the base fee per blob gas for the current block based on it's parent excess blob gas
pub fn calculate_base_fee_per_blob_gas(parent_excess_blob_gas: u64) -> u64 {
    fake_exponential(
        MIN_BASE_FEE_PER_BLOB_GAS,
        parent_excess_blob_gas,
        BLOB_BASE_FEE_UPDATE_FRACTION,
    )
}

// Defined in [EIP-4844](https://eips.ethereum.org/EIPS/eip-4844)
fn fake_exponential(factor: u64, numerator: u64, denominator: u64) -> u64 {
    let mut i = 1;
    let mut output = 0;
    let mut numerator_accum = factor * denominator;
    while numerator_accum > 0 {
        output += numerator_accum;
        numerator_accum = numerator_accum * numerator / (denominator * i);
        i += 1;
    }
    output / denominator
}

// Calculates the base fee for the current block based on its gas_limit and parent's gas and fee
// Returns None if the block gas limit is not valid in relation to its parent's gas limit
pub fn calculate_base_fee_per_gas(
    block_gas_limit: u64,
    parent_gas_limit: u64,
    parent_gas_used: u64,
    parent_base_fee_per_gas: u64,
) -> Option<u64> {
    // Check gas limit, if the check passes we can also rest assured that none of the
    // following divisions will have zero as a divider
    if !check_gas_limit(block_gas_limit, parent_gas_limit) {
        return None;
    }

    let parent_gas_target = parent_gas_limit / ELASTICITY_MULTIPLIER;

    Some(match parent_gas_used.cmp(&parent_gas_target) {
        Ordering::Equal => parent_base_fee_per_gas,
        Ordering::Greater => {
            let gas_used_delta = parent_gas_used - parent_gas_target;

            let parent_fee_gas_delta = parent_base_fee_per_gas * gas_used_delta;
            let target_fee_gas_delta = parent_fee_gas_delta / parent_gas_target;

            let base_fee_per_gas_delta =
                max(target_fee_gas_delta / BASE_FEE_MAX_CHANGE_DENOMINATOR, 1);

            parent_base_fee_per_gas + base_fee_per_gas_delta
        }
        Ordering::Less => {
            let gas_used_delta = parent_gas_target - parent_gas_used;

            let parent_fee_gas_delta = parent_base_fee_per_gas * gas_used_delta;
            let target_fee_gas_delta = parent_fee_gas_delta / parent_gas_target;

            let base_fee_per_gas_delta = target_fee_gas_delta / BASE_FEE_MAX_CHANGE_DENOMINATOR;

            parent_base_fee_per_gas - base_fee_per_gas_delta
        }
    })
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidBlockHeaderError {
    #[error("Gas used is greater than gas limit")]
    GasUsedGreaterThanGasLimit,
    #[error("Base fee per gas is incorrect")]
    BaseFeePerGasIncorrect,
    #[error("Timestamp is not greater than parent timestamp")]
    TimestampNotGreaterThanParent,
    #[error("Block number is not one greater than parent number")]
    BlockNumberNotOneGreater,
    #[error("Extra data is too long")]
    ExtraDataTooLong,
    #[error("Difficulty is not zero")]
    DifficultyNotZero,
    #[error("Nonce is not zero")]
    NonceNotZero,
    #[error("Ommers hash is not the default")]
    OmmersHashNotDefault,
    #[error("Parent hash is incorrect")]
    ParentHashIncorrect,
    // Cancun fork errors
    #[error("Excess blob gas is not present")]
    ExcessBlobGasNotPresent,
    #[error("Blob gas used is not present")]
    BlobGasUsedNotPresent,
    #[error("Excess blob gas is incorrect")]
    ExcessBlobGasIncorrect,
    #[error("Parent beacon block root is not present")]
    ParentBeaconBlockRootNotPresent,
    // Other fork errors
    #[error("Excess blob gas is present")]
    ExcessBlobGasPresent,
    #[error("Blob gas used is present")]
    BlobGasUsedPresent,
}

/// Validates that the header fields are correct in reference to the parent_header
pub fn validate_block_header(
    header: &BlockHeader,
    parent_header: &BlockHeader,
) -> Result<(), InvalidBlockHeaderError> {
    if header.gas_used > header.gas_limit {
        return Err(InvalidBlockHeaderError::GasUsedGreaterThanGasLimit);
    }
    let expected_base_fee_per_gas = if let Some(base_fee) = calculate_base_fee_per_gas(
        header.gas_limit,
        parent_header.gas_limit,
        parent_header.gas_used,
        parent_header.base_fee_per_gas.unwrap_or(INITIAL_BASE_FEE),
    ) {
        base_fee
    } else {
        return Err(InvalidBlockHeaderError::BaseFeePerGasIncorrect);
    };

    if expected_base_fee_per_gas != header.base_fee_per_gas.unwrap_or(INITIAL_BASE_FEE) {
        return Err(InvalidBlockHeaderError::BaseFeePerGasIncorrect);
    }

    if header.timestamp <= parent_header.timestamp {
        return Err(InvalidBlockHeaderError::TimestampNotGreaterThanParent);
    }

    if header.number != parent_header.number + 1 {
        return Err(InvalidBlockHeaderError::BlockNumberNotOneGreater);
    }

    if header.extra_data.len() > 32 {
        return Err(InvalidBlockHeaderError::ExtraDataTooLong);
    }

    if !header.difficulty.is_zero() {
        return Err(InvalidBlockHeaderError::DifficultyNotZero);
    }

    if header.nonce != 0 {
        return Err(InvalidBlockHeaderError::NonceNotZero);
    }

    if header.ommers_hash != *DEFAULT_OMMERS_HASH {
        return Err(InvalidBlockHeaderError::OmmersHashNotDefault);
    }

    if header.parent_hash != parent_header.compute_block_hash() {
        return Err(InvalidBlockHeaderError::ParentHashIncorrect);
    }

    Ok(())
}
/// Validates that excess_blob_gas and blob_gas_used are present in the header and
/// validates that excess_blob_gas value is correct on the block header
/// according to the values in the parent header.
pub fn validate_cancun_header_fields(
    header: &BlockHeader,
    parent_header: &BlockHeader,
) -> Result<(), InvalidBlockHeaderError> {
    if header.excess_blob_gas.is_none() {
        return Err(InvalidBlockHeaderError::ExcessBlobGasNotPresent);
    }
    if header.blob_gas_used.is_none() {
        return Err(InvalidBlockHeaderError::BlobGasUsedNotPresent);
    }
    if header.excess_blob_gas.unwrap() != calc_excess_blob_gas(parent_header) {
        return Err(InvalidBlockHeaderError::ExcessBlobGasIncorrect);
    }
    if header.parent_beacon_block_root.is_none() {
        return Err(InvalidBlockHeaderError::ParentBeaconBlockRootNotPresent);
    }
    Ok(())
}

/// Validates that the excess blob gas value is correct on the block header
/// according to the values in the parent header.
pub fn validate_no_cancun_header_fields(
    header: &BlockHeader,
) -> Result<(), InvalidBlockHeaderError> {
    if header.excess_blob_gas.is_some() {
        return Err(InvalidBlockHeaderError::ExcessBlobGasPresent);
    }
    if header.blob_gas_used.is_some() {
        return Err(InvalidBlockHeaderError::BlobGasUsedPresent);
    }
    Ok(())
}

fn calc_excess_blob_gas(parent_header: &BlockHeader) -> u64 {
    let parent_excess_blob_gas = parent_header.excess_blob_gas.unwrap_or_default();
    let parent_blob_gas_used = parent_header.blob_gas_used.unwrap_or_default();
    let parent_blob_gas = parent_excess_blob_gas + parent_blob_gas_used;

    if parent_blob_gas < 393216_u64 {
        0u64
    } else {
        parent_blob_gas - 393216_u64
    }
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use super::*;
    use ethereum_types::H160;
    use hex_literal::hex;

    #[test]
    fn test_compute_withdrawals_root() {
        // Source: https://github.com/ethereum/tests/blob/9760400e667eba241265016b02644ef62ab55de2/BlockchainTests/EIPTests/bc4895-withdrawals/amountIs0.json
        // "withdrawals" : [
        //             {
        //                 "address" : "0xc94f5374fce5edbc8e2a8697c15331677e6ebf0b",
        //                 "amount" : "0x00",
        //                 "index" : "0x00",
        //                 "validatorIndex" : "0x00"
        //             }
        //         ]
        // "withdrawalsRoot" : "0x48a703da164234812273ea083e4ec3d09d028300cd325b46a6a75402e5a7ab95"
        let withdrawals = vec![Withdrawal {
            index: 0x00,
            validator_index: 0x00,
            address: H160::from_slice(&hex!("c94f5374fce5edbc8e2a8697c15331677e6ebf0b")),
            amount: 0x00_u64,
        }];
        let expected_root = H256::from_slice(&hex!(
            "48a703da164234812273ea083e4ec3d09d028300cd325b46a6a75402e5a7ab95"
        ));
        let root = compute_withdrawals_root(&withdrawals);
        assert_eq!(root, expected_root);
    }

    #[test]
    fn test_validate_block_header() {
        let parent_block = BlockHeader {
            parent_hash: H256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000000",
            )
            .unwrap(),
            ommers_hash: H256::from_str(
                "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            )
            .unwrap(),
            coinbase: Address::zero(),
            state_root: H256::from_str(
                "0x590245a249decc317041b8dc7141cec0559c533efb82221e4e0a30a6456acf8b",
            )
            .unwrap(),
            transactions_root: H256::from_str(
                "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            )
            .unwrap(),
            receipts_root: H256::from_str(
                "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            )
            .unwrap(),
            logs_bloom: Bloom::from([0; 256]),
            difficulty: U256::zero(),
            number: 0,
            gas_limit: 0x016345785d8a0000,
            gas_used: 0,
            timestamp: 0,
            extra_data: Bytes::new(),
            prev_randao: H256::zero(),
            nonce: 0x0000000000000000,
            base_fee_per_gas: Some(0x07),
            withdrawals_root: Some(
                H256::from_str(
                    "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                )
                .unwrap(),
            ),
            blob_gas_used: Some(0x00),
            excess_blob_gas: Some(0x00),
            parent_beacon_block_root: Some(H256::zero()),
        };
        let block = BlockHeader {
            parent_hash: H256::from_str(
                "0x1ac1bf1eef97dc6b03daba5af3b89881b7ae4bc1600dc434f450a9ec34d44999",
            )
            .unwrap(),
            ommers_hash: H256::from_str(
                "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            )
            .unwrap(),
            coinbase: Address::from_str("0x2adc25665018aa1fe0e6bc666dac8fc2697ff9ba").unwrap(),
            state_root: H256::from_str(
                "0x9de6f95cb4ff4ef22a73705d6ba38c4b927c7bca9887ef5d24a734bb863218d9",
            )
            .unwrap(),
            transactions_root: H256::from_str(
                "0x578602b2b7e3a3291c3eefca3a08bc13c0d194f9845a39b6f3bcf843d9fed79d",
            )
            .unwrap(),
            receipts_root: H256::from_str(
                "0x035d56bac3f47246c5eed0e6642ca40dc262f9144b582f058bc23ded72aa72fa",
            )
            .unwrap(),
            logs_bloom: Bloom::from([0; 256]),
            difficulty: U256::zero(),
            number: 1,
            gas_limit: 0x016345785d8a0000,
            gas_used: 0xa8de,
            timestamp: 0x03e8,
            extra_data: Bytes::new(),
            prev_randao: H256::zero(),
            nonce: 0x0000000000000000,
            base_fee_per_gas: Some(0x07),
            withdrawals_root: Some(
                H256::from_str(
                    "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                )
                .unwrap(),
            ),
            blob_gas_used: Some(0x00),
            excess_blob_gas: Some(0x00),
            parent_beacon_block_root: Some(H256::zero()),
        };
        assert!(validate_block_header(&block, &parent_block).is_ok())
    }

    #[test]
    fn test_compute_transactions_root() {
        let encoded_transactions = [
            "0x01f8d68330182404842daf517a830186a08080b880c1597f3c842558e64df52c3e0f0973067577c030c0c6578dbb2eef63155a21106fd4426057527f296b2ecdfabc81e34ffc82e89dec20f6b7c41fa1969d3c3bc44262c86f08b5b76077527fb7ece918787c50c878052c30a8b1d4abc07331e6d14b8ded52bbc58a6e9992b76097527f0110937c38cc13b914f201fc09dc6f7a80c001a09930cb92b4a27dce971c697a8c47fa34c98d076abc7b36e1239d6abcfc7c8403a041b35118447fe77c38c0b3a92a2dd3ecba4a9e4b35cc6534cd787f56c0cf2e21",
            "0xf86e81fa843127403882f61894db8d964741c53e55df9c2d4e9414c6c96482874e870aa87bee538000808360306ca03aa421df67a101c45ff9cb06ce28f518a5d8d8dbb76a79361280071909650a27a05a447ff053c4ae601cfe81859b58d5603f2d0a73481c50f348089032feb0b073",
            "0x02f8ef83301824048413f157f8842daf517a830186a094000000000000000000000000000000000000000080b8807a0a600060a0553db8600060c855c77fb29ecd7661d8aefe101a0db652a728af0fded622ff55d019b545d03a7532932a60ad52604260cd5360bf60ce53609460cf53603e60d05360f560d153bc596000609e55600060c6556000601f556000609155535660556057536055605853606e60595360e7605a5360d0605b5360eb60c080a03acb03b1fc20507bc66210f7e18ff5af65038fb22c626ae488ad9513d9b6debca05d38459e9d2a221eb345b0c2761b719b313d062ff1ea3d10cf5b8762c44385a6",
            "0x01f8ea8330182402842daf517a830186a094000000000000000000000000000000000000000080b880bdb30d976000604e557145600060a155d67fe7e473caf6e33cba341136268fc1189ba07837ef8a266570289ff53afc43436260c7527f333dfe837f4838f6053e5e46e4151aeec28f356ec39a2db9769f36ec92e3e3f660e7527f0b261608674300d4621eff679096a6ed786591aca69f2b22a3ea6949621daade610107527f3cc080a01f3f906540fb56b0576c51b3ffa86df213fd1f407378c9441cfdd9d5f3c1df3da035691b16c053b68ec74683ae020293cbc6a47ac773dc8defb96cb680c576e5a3"
        ];
        let transactions: Vec<Transaction> = encoded_transactions
            .iter()
            .map(|hex| {
                Transaction::decode_canonical(&hex::decode(hex.trim_start_matches("0x")).unwrap())
                    .unwrap()
            })
            .collect();
        let transactions_root = compute_transactions_root(&transactions);
        let expected_root = H256::from_slice(
            &hex::decode("adf0387d2303fe80aeca23bf6828c979b44d8a8fe4a1ba1d3511bc1567ca80de")
                .unwrap(),
        );
        assert_eq!(transactions_root, expected_root);
    }
}
