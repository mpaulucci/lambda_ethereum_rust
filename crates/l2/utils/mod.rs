use keccak_hash::H256;
use libsecp256k1::SecretKey;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod config;
pub mod engine_client;
pub mod eth_client;

pub fn secret_key_deserializer<'de, D>(deserializer: D) -> Result<SecretKey, D::Error>
where
    D: Deserializer<'de>,
{
    let hex = H256::deserialize(deserializer)?;
    SecretKey::parse(hex.as_fixed_bytes()).map_err(serde::de::Error::custom)
}

pub fn secret_key_serializer<S>(secret_key: &SecretKey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let hex = H256::from_slice(&secret_key.serialize());
    hex.serialize(serializer)
}
