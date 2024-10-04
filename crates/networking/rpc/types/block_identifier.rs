use std::{fmt::Display, str::FromStr};

use ethereum_rust_core::types::{BlockHash, BlockHeader, BlockNumber};
use ethereum_rust_storage::{error::StoreError, Store};
use serde::Deserialize;
use serde_json::Value;

use crate::utils::RpcErr;

#[derive(Clone, Debug)]
pub enum BlockIdentifier {
    Number(BlockNumber),
    Tag(BlockTag),
}

#[derive(Clone, Debug)]
pub enum BlockIdentifierOrHash {
    Hash(BlockHash),
    Identifier(BlockIdentifier),
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BlockTag {
    Earliest,
    Finalized,
    Safe,
    #[default]
    Latest,
    Pending,
}

impl BlockIdentifier {
    pub fn resolve_block_number(&self, storage: &Store) -> Result<Option<BlockNumber>, StoreError> {
        match self {
            BlockIdentifier::Number(num) => Ok(Some(*num)),
            BlockIdentifier::Tag(tag) => match tag {
                BlockTag::Earliest => storage.get_earliest_block_number(),
                BlockTag::Finalized => storage.get_finalized_block_number(),
                BlockTag::Safe => storage.get_safe_block_number(),
                BlockTag::Latest => storage.get_latest_block_number(),
                BlockTag::Pending => storage.get_pending_block_number(),
            },
        }
    }

    pub fn parse(serde_value: Value, arg_index: u64) -> Result<Self, RpcErr> {
        // Check if it is a BlockTag
        if let Ok(tag) = serde_json::from_value::<BlockTag>(serde_value.clone()) {
            return Ok(BlockIdentifier::Tag(tag));
        };
        // Parse BlockNumber
        let Ok(hex_str) = serde_json::from_value::<String>(serde_value) else {
            return Err(RpcErr::BadParams);
        };
        // Check that the BlockNumber is 0x prefixed
        let Some(hex_str) = hex_str.strip_prefix("0x") else {
            return Err(RpcErr::BadHexFormat(arg_index));
        };

        // Parse hex string
        let Ok(block_number) = u64::from_str_radix(hex_str, 16) else {
            return Err(RpcErr::BadHexFormat(arg_index));
        };
        Ok(BlockIdentifier::Number(block_number))
    }

    pub fn resolve_block_header(&self, storage: &Store) -> Result<Option<BlockHeader>, StoreError> {
        match self.resolve_block_number(storage)? {
            Some(block_number) => storage.get_block_header(block_number),
            _ => Ok(None),
        }
    }
}

impl BlockIdentifierOrHash {
    #[allow(unused)]
    pub fn resolve_block_number(&self, storage: &Store) -> Result<Option<BlockNumber>, StoreError> {
        match self {
            BlockIdentifierOrHash::Identifier(id) => id.resolve_block_number(storage),
            BlockIdentifierOrHash::Hash(block_hash) => storage.get_block_number(*block_hash),
        }
    }

    pub fn parse(serde_value: Value, arg_index: u64) -> Result<BlockIdentifierOrHash, RpcErr> {
        // Parse as BlockHash
        if let Some(block_hash) = serde_json::from_value::<String>(serde_value.clone())
            .ok()
            .and_then(|hex_str| BlockHash::from_str(&hex_str).ok())
        {
            Ok(BlockIdentifierOrHash::Hash(block_hash))
        } else {
            // Parse as BlockIdentifier
            BlockIdentifier::parse(serde_value, arg_index).map(BlockIdentifierOrHash::Identifier)
        }
    }

    #[allow(unused)]
    pub fn is_latest(&self, storage: &Store) -> Result<bool, StoreError> {
        if self == &BlockTag::Latest {
            return Ok(true);
        }

        let result = self.resolve_block_number(storage)?;
        let latest = storage.get_latest_block_number()?;
        match (result, latest) {
            (Some(result), Some(latest)) => Ok(result == latest),
            _ => Ok(false),
        }
    }
}

impl Display for BlockIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockIdentifier::Number(num) => num.fmt(f),
            BlockIdentifier::Tag(tag) => match tag {
                BlockTag::Earliest => "Earliest".fmt(f),
                BlockTag::Finalized => "Finalized".fmt(f),
                BlockTag::Safe => "Safe".fmt(f),
                BlockTag::Latest => "Latest".fmt(f),
                BlockTag::Pending => "Pending".fmt(f),
            },
        }
    }
}

impl Default for BlockIdentifier {
    fn default() -> BlockIdentifier {
        BlockIdentifier::Tag(BlockTag::default())
    }
}

impl Display for BlockIdentifierOrHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockIdentifierOrHash::Identifier(id) => id.fmt(f),
            BlockIdentifierOrHash::Hash(hash) => hash.fmt(f),
        }
    }
}

impl PartialEq<BlockTag> for BlockIdentifierOrHash {
    fn eq(&self, other: &BlockTag) -> bool {
        match self {
            BlockIdentifierOrHash::Identifier(BlockIdentifier::Tag(tag)) => tag == other,
            _ => false,
        }
    }
}
