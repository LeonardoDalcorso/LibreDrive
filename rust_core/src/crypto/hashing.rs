//! Content-addressable hashing using BLAKE3
//!
//! Provides fast, secure hashing for content addressing and integrity verification.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Content hash using BLAKE3 (32 bytes)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Hash data and return content hash
    pub fn hash(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Self(*hash.as_bytes())
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Convert to base58 string (shorter, URL-safe)
    pub fn to_base58(&self) -> String {
        bs58::encode(&self.0).into_string()
    }

    /// Parse from base58 string
    pub fn from_base58(s: &str) -> Result<Self, bs58::decode::Error> {
        let bytes = bs58::decode(s).into_vec()?;
        if bytes.len() != 32 {
            return Err(bs58::decode::Error::BufferTooSmall);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Verify that data matches this hash
    pub fn verify(&self, data: &[u8]) -> bool {
        Self::hash(data) == *self
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", self.to_base58())
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

/// Incremental hasher for large files
pub struct IncrementalHasher {
    hasher: Hasher,
    bytes_processed: u64,
}

impl IncrementalHasher {
    /// Create a new incremental hasher
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
            bytes_processed: 0,
        }
    }

    /// Update with more data
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.bytes_processed += data.len() as u64;
    }

    /// Finalize and get the content hash
    pub fn finalize(self) -> ContentHash {
        ContentHash(*self.hasher.finalize().as_bytes())
    }

    /// Get bytes processed so far
    pub fn bytes_processed(&self) -> u64 {
        self.bytes_processed
    }
}

impl Default for IncrementalHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash a file in chunks (for large files)
pub fn hash_file_chunked<R: std::io::Read>(reader: &mut R, chunk_size: usize) -> std::io::Result<ContentHash> {
    let mut hasher = IncrementalHasher::new();
    let mut buffer = vec![0u8; chunk_size];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

/// Merkle tree node for verifying file chunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    /// Root hash of the tree
    pub root: ContentHash,

    /// Leaf hashes (one per chunk)
    pub leaves: Vec<ContentHash>,
}

impl MerkleTree {
    /// Build a Merkle tree from chunks
    pub fn build(chunks: &[&[u8]]) -> Self {
        let leaves: Vec<ContentHash> = chunks.iter().map(|c| ContentHash::hash(c)).collect();
        let root = Self::compute_root(&leaves);

        Self { root, leaves }
    }

    /// Compute root hash from leaves
    fn compute_root(leaves: &[ContentHash]) -> ContentHash {
        if leaves.is_empty() {
            return ContentHash::hash(&[]);
        }
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut current_level = leaves.to_vec();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for pair in current_level.chunks(2) {
                let combined = if pair.len() == 2 {
                    let mut data = Vec::with_capacity(64);
                    data.extend_from_slice(pair[0].as_bytes());
                    data.extend_from_slice(pair[1].as_bytes());
                    ContentHash::hash(&data)
                } else {
                    // Odd number of nodes, promote the last one
                    pair[0]
                };
                next_level.push(combined);
            }

            current_level = next_level;
        }

        current_level[0]
    }

    /// Verify a chunk at given index
    pub fn verify_chunk(&self, chunk: &[u8], index: usize) -> bool {
        if index >= self.leaves.len() {
            return false;
        }

        let chunk_hash = ContentHash::hash(chunk);
        chunk_hash == self.leaves[index]
    }

    /// Get the number of leaves
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash() {
        let data = b"Hello, CloudP2P!";
        let hash = ContentHash::hash(data);

        assert!(hash.verify(data));
        assert!(!hash.verify(b"Different data"));
    }

    #[test]
    fn test_hash_serialization() {
        let hash = ContentHash::hash(b"test data");

        let hex = hash.to_hex();
        let from_hex = ContentHash::from_hex(&hex).unwrap();
        assert_eq!(hash, from_hex);

        let base58 = hash.to_base58();
        let from_base58 = ContentHash::from_base58(&base58).unwrap();
        assert_eq!(hash, from_base58);
    }

    #[test]
    fn test_incremental_hasher() {
        let data = b"Hello, CloudP2P! This is a longer message for testing.";

        // Hash all at once
        let hash1 = ContentHash::hash(data);

        // Hash incrementally
        let mut hasher = IncrementalHasher::new();
        hasher.update(&data[..10]);
        hasher.update(&data[10..30]);
        hasher.update(&data[30..]);
        let hash2 = hasher.finalize();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_merkle_tree() {
        let chunks: Vec<&[u8]> = vec![
            b"chunk 1",
            b"chunk 2",
            b"chunk 3",
            b"chunk 4",
        ];

        let tree = MerkleTree::build(&chunks);

        assert_eq!(tree.leaf_count(), 4);
        assert!(tree.verify_chunk(b"chunk 1", 0));
        assert!(tree.verify_chunk(b"chunk 2", 1));
        assert!(!tree.verify_chunk(b"wrong chunk", 0));
    }

    #[test]
    fn test_merkle_tree_odd_chunks() {
        let chunks: Vec<&[u8]> = vec![
            b"chunk 1",
            b"chunk 2",
            b"chunk 3",
        ];

        let tree = MerkleTree::build(&chunks);

        assert_eq!(tree.leaf_count(), 3);
        assert!(tree.verify_chunk(b"chunk 3", 2));
    }
}
