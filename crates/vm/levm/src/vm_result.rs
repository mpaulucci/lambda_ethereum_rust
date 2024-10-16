use std::collections::HashMap;

use serde::{de, Deserialize};

use crate::{
    call_frame::Log,
    primitives::{Address, Bytes, H256, U256},
    vm::StorageSlot,
};

/// Errors that halt the program
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VMError {
    StackUnderflow,
    StackOverflow,
    InvalidJump,
    OpcodeNotAllowedInStaticContext,
    OpcodeNotFound,
    InvalidBytecode,
    OutOfGas,
    VeryLargeNumber,
    OverflowInArithmeticOp,
    FatalError,
    InvalidTransaction,
}

pub enum OpcodeSuccess {
    Continue,
    Result(ResultReason),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResultReason {
    Stop,
    Revert,
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultAndState {
    /// Status of execution
    pub result: ExecutionResult,
    /// State that got updated
    pub state: HashMap<Address, StateAccount>,
}

/// Result of a transaction execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecutionResult {
    /// Returned successfully
    Success {
        reason: SuccessReason,
        gas_used: u64,
        gas_refunded: u64,
        logs: Vec<Log>,
        output: Output,
        return_data: Bytes,
    },
    /// Reverted by `REVERT` opcode that doesn't spend all gas.
    Revert {
        reason: VMError,
        gas_used: u64,
        output: Bytes,
    },
    /// Reverted for various reasons and spend all gas.
    Halt {
        reason: VMError,
        /// Halting will spend all the gas, and will be equal to gas_limit.
        gas_used: u64,
    },
}

impl ExecutionResult {
    /// Returns true if transaction execution is successful.
    /// 1 indicates success, 0 indicates revert.
    /// <https://eips.ethereum.org/EIPS/eip-658>
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn is_revert(&self) -> bool {
        matches!(self, Self::Revert { .. })
    }

    /// Returns true if execution result is a Halt.
    pub fn is_halt(&self) -> bool {
        matches!(self, Self::Halt { .. })
    }

    /// Returns the output data of the execution.
    ///
    /// Returns `None` if the execution was halted.
    pub fn output(&self) -> Option<&Bytes> {
        let output = match self {
            Self::Success { output, .. } => Some(output.data()),
            Self::Revert { output, .. } => Some(output),
            _ => None,
        };
        output.and_then(|data| if data.is_empty() { None } else { Some(data) })
    }

    /// Consumes the type and returns the output data of the execution.
    ///
    /// Returns `None` if the execution was halted.
    pub fn into_output(self) -> Option<Bytes> {
        match self {
            Self::Success { output, .. } => Some(output.into_data()),
            Self::Revert { output, .. } => Some(output),
            _ => None,
        }
    }

    /// Returns the logs if execution is successful, or an empty list otherwise.
    pub fn logs(&self) -> &[Log] {
        match self {
            Self::Success { logs, .. } => logs,
            _ => &[],
        }
    }

    /// Consumes `self` and returns the logs if execution is successful, or an empty list otherwise.
    pub fn into_logs(self) -> Vec<Log> {
        match self {
            Self::Success { logs, .. } => logs,
            _ => Vec::new(),
        }
    }

    /// Returns the gas used.
    pub fn gas_used(&self) -> u64 {
        match *self {
            Self::Success { gas_used, .. }
            | Self::Revert { gas_used, .. }
            | Self::Halt { gas_used, .. } => gas_used,
        }
    }

    /// Returns the gas refunded.
    pub fn gas_refunded(&self) -> u64 {
        match *self {
            Self::Success { gas_refunded, .. } => gas_refunded,
            _ => 0,
        }
    }

    pub fn reason(&self) -> VMError {
        match self {
            Self::Revert { reason, .. } => reason.clone(),
            Self::Halt { reason, .. } => reason.clone(),
            _ => panic!("ExecutionResult.reason() called on non-revert/halt result"),
        }
    }
}

/// Output of a transaction execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Output {
    Call(Bytes),
    Create(Bytes, Option<Address>),
}

impl Output {
    /// Returns the output data of the execution output.
    pub fn into_data(self) -> Bytes {
        match self {
            Output::Call(data) => data,
            Output::Create(data, _) => data,
        }
    }

    /// Returns the output data of the execution output.
    pub fn data(&self) -> &Bytes {
        match self {
            Output::Call(data) => data,
            Output::Create(data, _) => data,
        }
    }

    /// Returns the created address, if any.
    pub fn address(&self) -> Option<&Address> {
        match self {
            Output::Call(_) => None,
            Output::Create(_, address) => address.as_ref(),
        }
    }
}

/// Transaction validation error.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InvalidTx {
    /// When using the EIP-1559 fee model introduced in the London upgrade, transactions specify two primary fee fields:
    /// - `gas_max_fee`: The maximum total fee a user is willing to pay, inclusive of both base fee and priority fee.
    /// - `gas_priority_fee`: The extra amount a user is willing to give directly to the miner, often referred to as the "tip".
    ///
    /// Provided `gas_priority_fee` exceeds the total `gas_max_fee`.
    PriorityFeeGreaterThanMaxFee,
    /// EIP-1559: `gas_price` is less than `basefee`.
    GasPriceLessThanBasefee,
    /// `gas_limit` in the tx is bigger than `block_gas_limit`.
    CallerGasLimitMoreThanBlock,
    /// Initial gas for a Call is bigger than `gas_limit`.
    ///
    /// Initial gas for a Call contains:
    /// - initial stipend gas
    /// - gas for access list and input data
    CallGasCostMoreThanGasLimit,
    /// EIP-3607 Reject transactions from senders with deployed code
    RejectCallerWithCode,
    /// Transaction account does not have enough amount of ether to cover transferred value and gas_limit*gas_price.
    LackOfFundForMaxFee {
        fee: Box<U256>,
        balance: Box<U256>,
    },
    /// Overflow payment in transaction.
    OverflowPaymentInTransaction,
    /// Nonce overflows in transaction.
    NonceOverflowInTransaction,
    NonceTooHigh {
        tx: u64,
        state: u64,
    },
    NonceTooLow {
        tx: u64,
        state: u64,
    },
    /// EIP-3860: Limit and meter initcode
    CreateInitCodeSizeLimit,
    /// Transaction chain id does not match the config chain id.
    InvalidChainId,
    /// Access list is not supported for blocks before the Berlin hardfork.
    AccessListNotSupported,
    /// `max_fee_per_blob_gas` is not supported for blocks before the Cancun hardfork.
    MaxFeePerBlobGasNotSupported,
    /// `blob_hashes`/`blob_versioned_hashes` is not supported for blocks before the Cancun hardfork.
    BlobVersionedHashesNotSupported,
    /// Block `blob_gas_price` is greater than tx-specified `max_fee_per_blob_gas` after Cancun.
    BlobGasPriceGreaterThanMax,
    /// There should be at least one blob in Blob transaction.
    EmptyBlobs,
    /// Blob transaction can't be a create transaction.
    /// `to` must be present
    BlobCreateTransaction,
    /// Transaction has more then [`crate::MAX_BLOB_NUMBER_PER_BLOCK`] blobs
    TooManyBlobs {
        max: usize,
        have: usize,
    },
    /// Blob transaction contains a versioned hash with an incorrect version
    BlobVersionNotSupported,
    // The excess blob gas is not set
    ExcessBlobGasNotSet,
    /// EOF crate should have `to` address
    EofCrateShouldHaveToAddress,
}

/// Reason a transaction successfully completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuccessReason {
    Stop,
    Return,
    SelfDestruct,
    EofReturnContract,
}

/// Indicates that the EVM has experienced an exceptional halt. This causes execution to
/// immediately end with all gas being consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HaltReason {
    OutOfGas(OutOfGasError),
    OpcodeNotFound,
    InvalidFEOpcode,
    InvalidJump,
    NotActivated,
    StackUnderflow,
    StackOverflow,
    OutOfOffset,
    CreateCollision,
    PrecompileError,
    NonceOverflow,
    /// Create init code size exceeds limit (runtime).
    CreateContractSizeLimit,
    /// Error on created contract that begins with EF
    CreateContractStartingWithEF,
    /// EIP-3860: Limit and meter initcode. Initcode size limit exceeded.
    CreateInitCodeSizeLimit,

    /* Internal Halts that can be only found inside Inspector */
    OverflowPayment,
    StateChangeDuringStaticCall,
    CallNotAllowedInsideStatic,
    OutOfFunds,
    CallTooDeep,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum OutOfGasError {
    // Basic OOG error
    Basic,
    // Tried to expand past REVM limit
    MemoryLimit,
    // Basic OOG error from memory expansion
    Memory,
    // When performing something that takes a U256 and casts down to a u64, if its too large this would fire
    // i.e. in `as_usize_or_fail`
    InvalidOperand,
    // When we try to create a transaction with the exact same code
    RecursiveCreate,
}

#[derive(Debug, PartialEq)]
pub enum PrecompileError {
    InvalidCalldata,
    NotEnoughGas,
    Secp256k1Error,
    InvalidEcPoint,
}

#[derive(Debug, Clone)]
pub enum ExitStatusCode {
    Return = 0,
    Stop,
    Revert,
    Error,
    Default,
}
impl ExitStatusCode {
    #[inline(always)]
    pub fn to_u8(self) -> u8 {
        self as u8
    }
    pub fn from_u8(value: u8) -> Self {
        match value {
            x if x == Self::Return.to_u8() => Self::Return,
            x if x == Self::Stop.to_u8() => Self::Stop,
            x if x == Self::Revert.to_u8() => Self::Revert,
            x if x == Self::Error.to_u8() => Self::Error,
            _ => Self::Default,
        }
    }
}

pub fn deserialize_str_as_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;

    if let Some(stripped) = string.strip_prefix("0x") {
        u64::from_str_radix(stripped, 16)
    } else {
        string.parse()
    }
    .map_err(serde::de::Error::custom)
}

#[derive(Clone, Default, PartialEq, Eq, Debug, Deserialize)]
pub struct AccountInfo {
    pub balance: U256,
    pub nonce: u64,
    pub code_hash: H256,
    pub code: Bytes,
}
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct StateAccount {
    pub info: AccountInfo,
    pub storage: HashMap<U256, StorageSlot>,
    pub status: AccountStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum AccountStatus {
    /// When account is loaded but not touched or interacted with.
    /// This is the default state.
    #[default]
    Loaded = 0b00000000,
    /// When account is newly created we will not access database
    /// to fetch storage values
    Created = 0b00000001,
    /// If account is marked for self destruction.
    SelfDestructed = 0b00000010,
    /// Only when account is marked as touched we will save it to database.
    Touched = 0b00000100,
    /// used only for pre spurious dragon hardforks where existing and empty were two separate states.
    /// it became same state after EIP-161: State trie clearing
    LoadedAsNotExisting = 0b0001000,
    /// used to mark account as cold
    Cold = 0b0010000,
}
