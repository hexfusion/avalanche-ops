use std::{
    cmp::Ordering,
    fmt,
    fs::File,
    hash::{Hash, Hasher},
    io::{self, BufReader, Error, ErrorKind},
    path::Path,
    str::FromStr,
    string::String,
};

use lazy_static::lazy_static;
use log::{info, warn};
use rustls_pemfile::{read_one, Item};
use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

use crate::{formatting, packer, soft_key};
use utils::hash;

pub const ID_LEN: usize = 32;
pub const SHORT_ID_LEN: usize = 20;

pub const NODE_ID_LEN: usize = 20;
pub const NODE_ID_ENCODE_PREFIX: &str = "NodeID-";

lazy_static! {
    static ref EMPTY: Vec<u8> = vec![0; ID_LEN];
    static ref SHORT_EMPTY: Vec<u8> = vec![0; SHORT_ID_LEN];
    static ref NODE_ID_EMPTY: Vec<u8> = vec![0; NODE_ID_LEN];
}

/// ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/ids#ID
#[derive(Debug, Deserialize, Clone, Eq)]
pub struct Id {
    pub d: Vec<u8>,
}

impl Default for Id {
    fn default() -> Self {
        Self::default()
    }
}

impl Id {
    pub fn default() -> Self {
        Self { d: EMPTY.to_vec() }
    }

    pub fn empty() -> Self {
        Self { d: EMPTY.to_vec() }
    }

    pub fn is_empty(&self) -> bool {
        (*self) == Self::empty()
    }

    /// If the passed array is shorter than the ID_LEN,
    /// it fills in with zero.
    pub fn from_slice(d: &[u8]) -> Self {
        assert!(d.len() <= ID_LEN);
        let mut d: Vec<u8> = Vec::from(d);
        if d.len() < ID_LEN {
            d.resize(ID_LEN, 0);
        }
        Self { d }
    }

    /// ref. "ids.ID.Prefix(output_index)"
    pub fn prefix(&self, prefixes: &[u64]) -> Self {
        let n = prefixes.len() + packer::U64_LEN + 32;
        let packer = packer::Packer::new(n, n);
        for pfx in prefixes {
            packer.pack_u64(*pfx);
        }
        packer.pack_bytes(&self.d);

        let b = packer.take_bytes();
        let d = hash::compute_sha256(&b);
        Self::from_slice(&d)
    }
}

/// ref. https://doc.rust-lang.org/std/string/trait.ToString.html
/// ref. https://doc.rust-lang.org/std/fmt/trait.Display.html
/// Use "Self.to_string()" to directly invoke this
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = formatting::encode_cb58_with_checksum(&self.d);
        write!(f, "{}", s)
    }
}

/// ref. https://doc.rust-lang.org/std/str/trait.FromStr.html
impl FromStr for Id {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decoded = formatting::decode_cb58_with_checksum(s).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed decode_cb58_with_checksum '{}'", e),
            )
        })?;
        Ok(Self::from_slice(&decoded))
    }
}

/// ref. https://serde.rs/impl-serialize.html
impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn fmt_id<'de, D>(deserializer: D) -> Result<Id, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Id::from_str(&s).map_err(serde::de::Error::custom)
}

pub fn deserialize_id<'de, D>(deserializer: D) -> Result<Option<Id>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "fmt_id")] Id);
    let v = Option::deserialize(deserializer)?;
    Ok(v.map(|Wrapper(a)| a))
}

pub fn must_deserialize_id<'de, D>(deserializer: D) -> Result<Id, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "fmt_id")] Id);
    let v = Option::deserialize(deserializer)?;
    match v.map(|Wrapper(a)| a) {
        Some(unwrapped) => Ok(unwrapped),
        None => Err(serde::de::Error::custom("empty Id from deserialization")),
    }
}

/// RUST_LOG=debug cargo test --package avalanche-types --lib -- ids::test_id --exact --show-output
/// ref. "avalanchego/ids.TestIDMarshalJSON"
#[test]
fn test_id() {
    let id = Id::from_slice(&<Vec<u8>>::from([
        0x3d, 0x0a, 0xd1, 0x2b, 0x8e, 0xe8, 0x92, 0x8e, 0xdf, 0x24, //
        0x8c, 0xa9, 0x1c, 0xa5, 0x56, 0x00, 0xfb, 0x38, 0x3f, 0x07, //
        0xc3, 0x2b, 0xff, 0x1d, 0x6d, 0xec, 0x47, 0x2b, 0x25, 0xcf, //
        0x59, 0xa7,
    ]));
    assert_eq!(
        id.to_string(),
        "TtF4d2QWbk5vzQGTEPrN48x6vwgAoAmKQ9cbp79inpQmcRKES"
    );
    let id_from_str = Id::from_str("TtF4d2QWbk5vzQGTEPrN48x6vwgAoAmKQ9cbp79inpQmcRKES").unwrap();
    assert_eq!(id, id_from_str);

    let id = Id::from_slice(&<Vec<u8>>::from([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        0x00, 0x00,
    ]));
    assert_eq!(id.to_string(), "11111111111111111111111111111111LpoYY");
    let id_from_str = Id::from_str("11111111111111111111111111111111LpoYY").unwrap();
    assert_eq!(id, id_from_str);
}

impl Ord for Id {
    fn cmp(&self, other: &Id) -> Ordering {
        self.d.cmp(&(other.d))
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Id) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Id {
    fn eq(&self, other: &Id) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// ref. https://rust-lang.github.io/rust-clippy/master/index.html#derive_hash_xor_eq
impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.d.hash(state);
    }
}

#[derive(Eq)]
pub struct Ids(Vec<Id>);

impl Ids {
    pub fn new(ids: &[Id]) -> Self {
        Ids(Vec::from(ids))
    }
}

impl Ord for Ids {
    fn cmp(&self, other: &Ids) -> Ordering {
        // packer encodes the array length first
        // so if the lengths differ, the ordering is decided
        let l1 = self.0.len();
        let l2 = other.0.len();
        l1.cmp(&l2) // returns when lengths are not Equal
            .then_with(
                || self.0.cmp(&other.0), // if lengths are Equal, compare the ids
            )
    }
}

impl PartialOrd for Ids {
    fn partial_cmp(&self, other: &Ids) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Ids {
    fn eq(&self, other: &Ids) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// RUST_LOG=debug cargo test --package avalanche-types --lib -- ids::test_sort_ids --exact --show-output
#[test]
fn test_sort_ids() {
    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = Id::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00]));
    let id2 = Id::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00, 0x00]));
    assert!(id1 == id2);

    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = Id::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00, 0x00]));
    let id2 = Id::from_slice(&<Vec<u8>>::from([0x02]));
    assert!(id1 < id2);

    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = Id::from_slice(&<Vec<u8>>::from([0x02, 0x00, 0x00, 0x00, 0x00]));
    let id2 = Id::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00, 0x00]));
    assert!(id1 > id2);

    // lengths of Ids matter
    let ids1 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x03])),
    ]);
    let ids2 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x03])),
    ]);
    assert!(ids1 == ids2);

    // lengths of Ids matter
    let ids1 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x05])),
        Id::from_slice(&<Vec<u8>>::from([0x06])),
        Id::from_slice(&<Vec<u8>>::from([0x07])),
    ]);
    let ids2 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x03])),
        Id::from_slice(&<Vec<u8>>::from([0x04])),
    ]);
    assert!(ids1 < ids2);

    // lengths of Ids matter
    let ids1 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x03])),
        Id::from_slice(&<Vec<u8>>::from([0x04])),
    ]);
    let ids2 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x09])),
        Id::from_slice(&<Vec<u8>>::from([0x09])),
        Id::from_slice(&<Vec<u8>>::from([0x09])),
    ]);
    assert!(ids1 > ids2);

    // lengths of Ids matter
    let ids1 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x03])),
    ]);
    let ids2 = Ids(vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x05])),
    ]);
    assert!(ids1 < ids2);

    let mut ids1 = vec![
        Id::from_slice(&<Vec<u8>>::from([0x03])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x01])),
    ];
    ids1.sort();
    let ids2 = vec![
        Id::from_slice(&<Vec<u8>>::from([0x01])),
        Id::from_slice(&<Vec<u8>>::from([0x02])),
        Id::from_slice(&<Vec<u8>>::from([0x03])),
    ];
    assert!(ids1 == ids2);
}

/// ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/ids#ShortID
#[derive(Debug, Deserialize, Clone, Eq)]
pub struct ShortId {
    pub d: Vec<u8>,
}

impl Default for ShortId {
    fn default() -> Self {
        Self::default()
    }
}

impl ShortId {
    pub fn default() -> Self {
        Self {
            d: SHORT_EMPTY.to_vec(),
        }
    }

    pub fn empty() -> Self {
        Self {
            d: SHORT_EMPTY.to_vec(),
        }
    }

    pub fn is_empty(&self) -> bool {
        (*self) == Self::empty()
    }

    pub fn from_slice(d: &[u8]) -> Self {
        assert!(d.len() <= SHORT_ID_LEN);
        let mut d: Vec<u8> = Vec::from(d);
        if d.len() < SHORT_ID_LEN {
            d.resize(SHORT_ID_LEN, 0);
        }
        Self { d }
    }
}

/// ref. https://doc.rust-lang.org/std/string/trait.ToString.html
/// ref. https://doc.rust-lang.org/std/fmt/trait.Display.html
/// Use "Self.to_string()" to directly invoke this
impl fmt::Display for ShortId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = formatting::encode_cb58_with_checksum(&self.d);
        write!(f, "{}", s)
    }
}

/// ref. https://doc.rust-lang.org/std/str/trait.FromStr.html
impl FromStr for ShortId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decoded = formatting::decode_cb58_with_checksum(s).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed decode_cb58_with_checksum '{}'", e),
            )
        })?;
        Ok(Self::from_slice(&decoded))
    }
}

/// ref. https://serde.rs/impl-serialize.html
impl Serialize for ShortId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn fmt_short_id<'de, D>(deserializer: D) -> Result<ShortId, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    ShortId::from_str(&s).map_err(serde::de::Error::custom)
}

pub fn deserialize_short_id<'de, D>(deserializer: D) -> Result<Option<ShortId>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "fmt_short_id")] ShortId);
    let v = Option::deserialize(deserializer)?;
    Ok(v.map(|Wrapper(a)| a))
}

pub fn must_deserialize_short_id<'de, D>(deserializer: D) -> Result<ShortId, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "fmt_short_id")] ShortId);
    let v = Option::deserialize(deserializer)?;
    match v.map(|Wrapper(a)| a) {
        Some(unwrapped) => Ok(unwrapped),
        None => Err(serde::de::Error::custom(
            "empty ShortId from deserialization",
        )),
    }
}

/// RUST_LOG=debug cargo test --package avalanche-types --lib -- ids::test_short_id --exact --show-output
#[test]
fn test_short_id() {
    let id = ShortId::from_slice(&<Vec<u8>>::from([
        0x3d, 0x0a, 0xd1, 0x2b, 0x8e, 0xe8, 0x92, 0x8e, 0xdf, 0x24, //
        0x8c, 0xa9, 0x1c, 0xa5, 0x56, 0x00, 0xfb, 0x38, 0x3f, 0x07, //
    ]));
    assert_eq!(id.to_string(), "6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx");
    let id_from_str = ShortId::from_str("6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx").unwrap();
    assert_eq!(id, id_from_str);
}

impl Ord for ShortId {
    fn cmp(&self, other: &ShortId) -> Ordering {
        self.d.cmp(&(other.d))
    }
}

impl PartialOrd for ShortId {
    fn partial_cmp(&self, other: &ShortId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ShortId {
    fn eq(&self, other: &ShortId) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// ref. https://rust-lang.github.io/rust-clippy/master/index.html#derive_hash_xor_eq
impl Hash for ShortId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.d.hash(state);
    }
}

#[derive(Eq)]
pub struct ShortIds(Vec<ShortId>);

impl ShortIds {
    pub fn new(ids: &[ShortId]) -> Self {
        ShortIds(Vec::from(ids))
    }
}

impl Ord for ShortIds {
    fn cmp(&self, other: &ShortIds) -> Ordering {
        // packer encodes the array length first
        // so if the lengths differ, the ordering is decided
        let l1 = self.0.len();
        let l2 = other.0.len();
        l1.cmp(&l2) // returns when lengths are not Equal
            .then_with(
                || self.0.cmp(&other.0), // if lengths are Equal, compare the ids
            )
    }
}

impl PartialOrd for ShortIds {
    fn partial_cmp(&self, other: &ShortIds) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ShortIds {
    fn eq(&self, other: &ShortIds) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// RUST_LOG=debug cargo test --package avalanche-types --lib -- ids::test_sort_short_ids --exact --show-output
#[test]
fn test_sort_short_ids() {
    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = ShortId::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00]));
    let id2 = ShortId::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00, 0x00]));
    assert!(id1 == id2);

    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = ShortId::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00, 0x00]));
    let id2 = ShortId::from_slice(&<Vec<u8>>::from([0x02]));
    assert!(id1 < id2);

    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = ShortId::from_slice(&<Vec<u8>>::from([0x02, 0x00, 0x00, 0x00, 0x00]));
    let id2 = ShortId::from_slice(&<Vec<u8>>::from([0x01, 0x00, 0x00, 0x00, 0x00]));
    assert!(id1 > id2);

    // lengths of ShortIds matter
    let ids1 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
    ]);
    let ids2 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
    ]);
    assert!(ids1 == ids2);

    // lengths of ShortIds matter
    let ids1 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x05])),
        ShortId::from_slice(&<Vec<u8>>::from([0x06])),
        ShortId::from_slice(&<Vec<u8>>::from([0x07])),
    ]);
    let ids2 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
        ShortId::from_slice(&<Vec<u8>>::from([0x04])),
    ]);
    assert!(ids1 < ids2);

    // lengths of ShortIds matter
    let ids1 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
        ShortId::from_slice(&<Vec<u8>>::from([0x04])),
    ]);
    let ids2 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x09])),
        ShortId::from_slice(&<Vec<u8>>::from([0x09])),
        ShortId::from_slice(&<Vec<u8>>::from([0x09])),
    ]);
    assert!(ids1 > ids2);

    // lengths of ShortIds matter
    let ids1 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
    ]);
    let ids2 = ShortIds(vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x05])),
    ]);
    assert!(ids1 < ids2);

    let mut ids1 = vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
    ];
    ids1.sort();
    let ids2 = vec![
        ShortId::from_slice(&<Vec<u8>>::from([0x01])),
        ShortId::from_slice(&<Vec<u8>>::from([0x02])),
        ShortId::from_slice(&<Vec<u8>>::from([0x03])),
    ];
    assert!(ids1 == ids2);
}

/// ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/ids#ShortID
#[derive(Debug, Deserialize, Clone, Eq)]
pub struct NodeId {
    pub d: Vec<u8>,
}

impl Default for NodeId {
    fn default() -> Self {
        Self::default()
    }
}

impl NodeId {
    pub fn default() -> Self {
        Self {
            d: NODE_ID_EMPTY.to_vec(),
        }
    }

    pub fn empty() -> Self {
        Self {
            d: NODE_ID_EMPTY.to_vec(),
        }
    }

    pub fn is_empty(&self) -> bool {
        (*self) == Self::empty()
    }

    pub fn from_slice(d: &[u8]) -> Self {
        assert_eq!(d.len(), SHORT_ID_LEN);
        let d = Vec::from(d);
        Self { d }
    }

    /// Loads a node ID from the PEM-encoded X509 certificate.
    /// ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/node#Node.Initialize
    pub fn from_cert_file(cert_file_path: &str) -> io::Result<Self> {
        info!("loading node ID from certificate {}", cert_file_path);
        if !Path::new(cert_file_path).exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("cert path {} does not exists", cert_file_path),
            ));
        }

        // ref. "tls.Certificate.Leaf.Raw" in Go
        // ref. "tls.X509KeyPair"
        // ref. "x509.ParseCertificate/parseCertificate"
        // ref. "x509.Certificate.Leaf"
        //
        // use openssl::x509::X509;
        // let pub_key_contents = fs::read(cert_file_path)?;
        // let pub_key = X509::from_pem(&pub_key_contents.to_vec())?;
        // let pub_key_der = pub_key.to_der()?;
        //
        // use pem;
        // let pub_key_contents = fs::read(cert_file_path)?;
        // let pub_key = pem::parse(&pub_key_contents.to_vec()).unwrap();
        // let pub_key_der = pub_key.contents;

        let pub_key_file = File::open(cert_file_path)?;
        let mut reader = BufReader::new(pub_key_file);
        let pem_read = read_one(&mut reader)?;
        let cert = {
            match pem_read.unwrap() {
                Item::X509Certificate(cert) => Some(cert),
                Item::RSAKey(_) | Item::PKCS8Key(_) | Item::ECKey(_) => {
                    warn!("cert path {} has unexpected private key", cert_file_path);
                    None
                }
                _ => None,
            }
        };
        if cert.is_none() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("cert path {} found no cert", cert_file_path),
            ));
        }

        // "ids.ToShortID(hashing.PubkeyBytesToAddress(StakingTLSCert.Leaf.Raw))"
        // ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/node#Node.Initialize
        let pub_key_der = cert.unwrap();
        Self::from_cert_raw(&pub_key_der)
    }

    /// Encodes the cert raw bytes to a node ID.
    /// It applies "sha256" and "ripemd160" on "Certificate.Leaf.Raw".
    /// ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/utils/hashing#PubkeyBytesToAddress
    /// ref. https://pkg.go.dev/github.com/ava-labs/avalanchego/ids#ToShortID
    pub fn from_cert_raw(cert_raw: &[u8]) -> io::Result<Self> {
        let short_address = soft_key::bytes_to_short_address_bytes(cert_raw)?;
        let node_id = Self::from_slice(&short_address);
        Ok(node_id)
    }

    pub fn short_id(&self) -> ShortId {
        ShortId::from_slice(&self.d)
    }
}

/// ref. https://doc.rust-lang.org/std/string/trait.ToString.html
/// ref. https://doc.rust-lang.org/std/fmt/trait.Display.html
/// Use "Self.to_string()" to directly invoke this
impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut node_id = String::from(NODE_ID_ENCODE_PREFIX);
        let short_id = formatting::encode_cb58_with_checksum(&self.d);
        node_id.push_str(&short_id);
        write!(f, "{}", node_id)
    }
}

/// ref. https://doc.rust-lang.org/std/str/trait.FromStr.html
impl FromStr for NodeId {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let processed = strip_node_id_prefix(s);
        let decoded = formatting::decode_cb58_with_checksum(processed).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("failed decode_cb58_with_checksum '{}'", e),
            )
        })?;
        Ok(Self::from_slice(&decoded))
    }
}

/// ref. https://serde.rs/impl-serialize.html
impl Serialize for NodeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn fmt_node_id<'de, D>(deserializer: D) -> Result<NodeId, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    NodeId::from_str(&s).map_err(serde::de::Error::custom)
}

pub fn deserialize_node_id<'de, D>(deserializer: D) -> Result<Option<NodeId>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "fmt_node_id")] NodeId);
    let v = Option::deserialize(deserializer)?;
    Ok(v.map(|Wrapper(a)| a))
}

pub fn must_deserialize_node_id<'de, D>(deserializer: D) -> Result<NodeId, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "fmt_node_id")] NodeId);
    let v = Option::deserialize(deserializer)?;
    match v.map(|Wrapper(a)| a) {
        Some(unwrapped) => Ok(unwrapped),
        None => Err(serde::de::Error::custom(
            "empty NodeId from deserialization",
        )),
    }
}

/// RUST_LOG=debug cargo test --package avalanche-types --lib -- ids::test_from_cert_file --exact --show-output
#[test]
fn test_from_cert_file() {
    let _ = env_logger::builder().is_test(true).try_init();

    let node_id = NodeId::from_slice(&<Vec<u8>>::from([
        0x3d, 0x0a, 0xd1, 0x2b, 0x8e, 0xe8, 0x92, 0x8e, 0xdf, 0x24, //
        0x8c, 0xa9, 0x1c, 0xa5, 0x56, 0x00, 0xfb, 0x38, 0x3f, 0x07, //
    ]));
    assert_eq!(
        format!("{}", node_id),
        "NodeID-6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx"
    );
    assert_eq!(
        node_id.short_id().to_string(),
        "6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-6ZmBHXTqjknJoZtXbnJ6x7af863rXDTwx").unwrap()
    );

    // copied from "avalanchego/staking/local/staking1.key,crt"
    // verified by "avalanchego-compatibility/node-id" for compatibility with Go
    let node_id = NodeId::from_cert_file("./artifacts/staker1.insecure.crt").unwrap();
    assert_eq!(
        format!("{}", node_id),
        "NodeID-7Xhw2mDxuDS44j42TCB6U5579esbSt3Lg"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-7Xhw2mDxuDS44j42TCB6U5579esbSt3Lg"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("7Xhw2mDxuDS44j42TCB6U5579esbSt3Lg").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-7Xhw2mDxuDS44j42TCB6U5579esbSt3Lg").unwrap()
    );

    let node_id = NodeId::from_cert_file("./artifacts/staker2.insecure.crt").unwrap();
    assert_eq!(
        format!("{}", node_id),
        "NodeID-MFrZFVCXPv5iCn6M9K6XduxGTYp891xXZ"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-MFrZFVCXPv5iCn6M9K6XduxGTYp891xXZ"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("MFrZFVCXPv5iCn6M9K6XduxGTYp891xXZ").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-MFrZFVCXPv5iCn6M9K6XduxGTYp891xXZ").unwrap()
    );

    let node_id = NodeId::from_cert_file("./artifacts/staker3.insecure.crt").unwrap();
    assert_eq!(
        format!("{}", node_id),
        "NodeID-NFBbbJ4qCmNaCzeW7sxErhvWqvEQMnYcN"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-NFBbbJ4qCmNaCzeW7sxErhvWqvEQMnYcN"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NFBbbJ4qCmNaCzeW7sxErhvWqvEQMnYcN").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-NFBbbJ4qCmNaCzeW7sxErhvWqvEQMnYcN").unwrap()
    );

    let node_id = NodeId::from_cert_file("./artifacts/staker4.insecure.crt").unwrap();
    assert_eq!(
        format!("{}", node_id),
        "NodeID-GWPcbFJZFfZreETSoWjPimr846mXEKCtu"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-GWPcbFJZFfZreETSoWjPimr846mXEKCtu"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("GWPcbFJZFfZreETSoWjPimr846mXEKCtu").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-GWPcbFJZFfZreETSoWjPimr846mXEKCtu").unwrap()
    );

    let node_id = NodeId::from_cert_file("./artifacts/staker5.insecure.crt").unwrap();
    assert_eq!(
        format!("{}", node_id),
        "NodeID-P7oB2McjBGgW2NXXWVYjV8JEDFoW9xDE5"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-P7oB2McjBGgW2NXXWVYjV8JEDFoW9xDE5"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("P7oB2McjBGgW2NXXWVYjV8JEDFoW9xDE5").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-P7oB2McjBGgW2NXXWVYjV8JEDFoW9xDE5").unwrap()
    );

    let node_id = NodeId::from_cert_file("./artifacts/test.insecure.crt").unwrap();
    assert_eq!(
        format!("{}", node_id),
        "NodeID-29HTAG5cfN2fw79A67Jd5zY9drcT51EBG"
    );
    assert_eq!(
        node_id.to_string(),
        "NodeID-29HTAG5cfN2fw79A67Jd5zY9drcT51EBG"
    );
    assert_eq!(
        node_id,
        NodeId::from_str("29HTAG5cfN2fw79A67Jd5zY9drcT51EBG").unwrap()
    );
    assert_eq!(
        node_id,
        NodeId::from_str("NodeID-29HTAG5cfN2fw79A67Jd5zY9drcT51EBG").unwrap()
    );
}

impl Ord for NodeId {
    fn cmp(&self, other: &NodeId) -> Ordering {
        self.d.cmp(&(other.d))
    }
}

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &NodeId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for NodeId {
    fn eq(&self, other: &NodeId) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// ref. https://rust-lang.github.io/rust-clippy/master/index.html#derive_hash_xor_eq
impl Hash for NodeId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.d.hash(state);
    }
}

#[derive(Eq)]
pub struct NodeIds(Vec<NodeId>);

impl NodeIds {
    pub fn new(ids: &[NodeId]) -> Self {
        NodeIds(Vec::from(ids))
    }
}

impl Ord for NodeIds {
    fn cmp(&self, other: &NodeIds) -> Ordering {
        // packer encodes the array length first
        // so if the lengths differ, the ordering is decided
        let l1 = self.0.len();
        let l2 = other.0.len();
        l1.cmp(&l2) // returns when lengths are not Equal
            .then_with(
                || self.0.cmp(&other.0), // if lengths are Equal, compare the ids
            )
    }
}

impl PartialOrd for NodeIds {
    fn partial_cmp(&self, other: &NodeIds) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for NodeIds {
    fn eq(&self, other: &NodeIds) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// RUST_LOG=debug cargo test --package avalanche-types --lib -- ids::test_sort_node_ids --exact --show-output
#[test]
fn test_sort_node_ids() {
    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = NodeId::from_slice(&<Vec<u8>>::from([
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]));
    let id2 = NodeId::from_slice(&<Vec<u8>>::from([
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]));
    assert!(id1 == id2);

    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = NodeId::from_slice(&<Vec<u8>>::from([
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]));
    let id2 = NodeId::from_slice(&<Vec<u8>>::from([
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]));
    assert!(id1 < id2);

    // lengths of individual ids do not matter since all are fixed-sized
    let id1 = NodeId::from_slice(&<Vec<u8>>::from([
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]));
    let id2 = NodeId::from_slice(&<Vec<u8>>::from([
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]));
    assert!(id1 > id2);

    // lengths of NodeIds matter
    let ids1 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    let ids2 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    assert!(ids1 == ids2);

    // lengths of NodeIds matter
    let ids1 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    let ids2 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    assert!(ids1 < ids2);

    // lengths of NodeIds matter
    let ids1 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    let ids2 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    assert!(ids1 > ids2);

    // lengths of NodeIds matter
    let ids1 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    let ids2 = NodeIds(vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ]);
    assert!(ids1 < ids2);

    let mut ids1 = vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ];
    ids1.sort();
    let ids2 = vec![
        NodeId::from_slice(&<Vec<u8>>::from([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
        NodeId::from_slice(&<Vec<u8>>::from([
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])),
    ];
    assert!(ids1 == ids2);
}

pub fn strip_node_id_prefix(addr: &str) -> &str {
    let n = NODE_ID_ENCODE_PREFIX.len();
    if &addr[0..n] == NODE_ID_ENCODE_PREFIX {
        &addr[n..]
    } else {
        addr
    }
}
