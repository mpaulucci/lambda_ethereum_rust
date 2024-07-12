use std::marker::PhantomData;

use ethereum_rust_core::{
    rlp::{decode::RLPDecode, encode::RLPEncode},
    types::{AccountInfo, BlockBody, BlockHeader, Receipt},
    Address,
};
use libmdbx::orm::{Decodable, Encodable};

// Account types
pub type AddressRLP = Rlp<Address>;
pub type AccountInfoRLP = Rlp<AccountInfo>;
pub type AccountCodeHashRLP = Rlp<Vec<u8>>;
pub type AccountCodeRLP = Rlp<Vec<u8>>;

// TODO: these structs were changed after a merge.
// See if we can reuse Rlp struct
pub struct AccountStorageKeyRLP(pub [u8; 32]);
pub struct AccountStorageValueRLP(pub [u8; 32]);

// Block types
pub type BlockHeaderRLP = Rlp<BlockHeader>;
pub type BlockBodyRLP = Rlp<BlockBody>;

// Receipt types
pub type ReceiptRLP = Rlp<Receipt>;

pub struct Rlp<T>(Vec<u8>, PhantomData<T>);

impl<T: RLPEncode> From<T> for Rlp<T> {
    fn from(value: T) -> Self {
        let mut buf = Vec::new();
        RLPEncode::encode(&value, &mut buf);
        Self(buf, Default::default())
    }
}

impl<T: RLPDecode> Rlp<T> {
    pub fn to(&self) -> T {
        T::decode(&self.0).unwrap()
    }
}

impl<T: Send + Sync> Decodable for Rlp<T> {
    fn decode(b: &[u8]) -> anyhow::Result<Self> {
        Ok(Rlp(b.to_vec(), Default::default()))
    }
}

impl<T: Send + Sync> Encodable for Rlp<T> {
    type Encoded = Vec<u8>;

    fn encode(self) -> Self::Encoded {
        self.0
    }
}

impl Encodable for AccountStorageKeyRLP {
    type Encoded = [u8; 32];

    fn encode(self) -> Self::Encoded {
        self.0
    }
}

impl Decodable for AccountStorageKeyRLP {
    fn decode(b: &[u8]) -> anyhow::Result<Self> {
        Ok(AccountStorageKeyRLP(b.try_into()?))
    }
}

impl Encodable for AccountStorageValueRLP {
    type Encoded = [u8; 32];

    fn encode(self) -> Self::Encoded {
        self.0
    }
}

impl Decodable for AccountStorageValueRLP {
    fn decode(b: &[u8]) -> anyhow::Result<Self> {
        Ok(AccountStorageValueRLP(b.try_into()?))
    }
}
