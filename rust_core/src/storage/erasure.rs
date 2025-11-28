//! Erasure Coding using Reed-Solomon
//!
//! Splits data into N shards where only K are needed to reconstruct.
//! This provides fault tolerance without full replication.

use super::StorageError;
use reed_solomon_erasure::galois_8::ReedSolomon;
use serde::{Deserialize, Serialize};

/// Configuration for erasure coding
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ErasureConfig {
    /// Number of data shards
    pub data_shards: usize,

    /// Number of parity shards
    pub parity_shards: usize,
}

impl ErasureConfig {
    /// Create a new erasure config
    pub fn new(data_shards: usize, parity_shards: usize) -> Self {
        Self {
            data_shards,
            parity_shards,
        }
    }

    /// Total number of shards
    pub fn total_shards(&self) -> usize {
        self.data_shards + self.parity_shards
    }

    /// Minimum shards needed to reconstruct
    pub fn min_shards(&self) -> usize {
        self.data_shards
    }

    /// Maximum shards that can be lost
    pub fn max_losses(&self) -> usize {
        self.parity_shards
    }

    /// Overhead factor (total / data)
    pub fn overhead(&self) -> f32 {
        self.total_shards() as f32 / self.data_shards as f32
    }
}

impl Default for ErasureConfig {
    fn default() -> Self {
        // 10 data shards + 4 parity shards = can lose up to 4 shards
        // Overhead: 1.4x (40% more storage for redundancy)
        Self {
            data_shards: 10,
            parity_shards: 4,
        }
    }
}

/// Erasure encoder - splits data into shards with parity
pub struct ErasureEncoder {
    config: ErasureConfig,
    rs: ReedSolomon,
}

impl ErasureEncoder {
    /// Create a new encoder
    pub fn new(config: ErasureConfig) -> Result<Self, StorageError> {
        let rs = ReedSolomon::new(config.data_shards, config.parity_shards)
            .map_err(|e| StorageError::ErasureCoding(e.to_string()))?;

        Ok(Self { config, rs })
    }

    /// Encode data into shards
    /// Returns Vec of (shard_index, shard_data)
    pub fn encode(&self, data: &[u8]) -> Result<Vec<Shard>, StorageError> {
        let shard_size = self.calculate_shard_size(data.len());

        // Prepare data shards (pad with zeros if needed)
        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(self.config.total_shards());

        for i in 0..self.config.data_shards {
            let start = i * shard_size;
            let end = (start + shard_size).min(data.len());

            let mut shard = if start < data.len() {
                data[start..end].to_vec()
            } else {
                vec![]
            };

            // Pad to shard_size
            shard.resize(shard_size, 0);
            shards.push(shard);
        }

        // Add empty parity shards
        for _ in 0..self.config.parity_shards {
            shards.push(vec![0u8; shard_size]);
        }

        // Convert to slices for reed_solomon
        let mut shard_refs: Vec<&mut [u8]> = shards.iter_mut().map(|s| s.as_mut_slice()).collect();

        // Encode (fills parity shards)
        self.rs
            .encode(&mut shard_refs)
            .map_err(|e| StorageError::ErasureCoding(e.to_string()))?;

        // Create shard objects with metadata
        let result: Vec<Shard> = shards
            .into_iter()
            .enumerate()
            .map(|(index, data)| {
                let size = data.len();
                Shard {
                    index,
                    data,
                    is_parity: index >= self.config.data_shards,
                    original_size: size,
                }
            })
            .collect();

        Ok(result)
    }

    /// Calculate shard size for given data length
    fn calculate_shard_size(&self, data_len: usize) -> usize {
        (data_len + self.config.data_shards - 1) / self.config.data_shards
    }

    /// Get the config
    pub fn config(&self) -> ErasureConfig {
        self.config
    }
}

/// Erasure decoder - reconstructs data from shards
pub struct ErasureDecoder {
    config: ErasureConfig,
    rs: ReedSolomon,
}

impl ErasureDecoder {
    /// Create a new decoder
    pub fn new(config: ErasureConfig) -> Result<Self, StorageError> {
        let rs = ReedSolomon::new(config.data_shards, config.parity_shards)
            .map_err(|e| StorageError::ErasureCoding(e.to_string()))?;

        Ok(Self { config, rs })
    }

    /// Decode shards back to original data
    /// Missing shards should be passed as None
    pub fn decode(
        &self,
        shards: Vec<Option<Shard>>,
        original_size: usize,
    ) -> Result<Vec<u8>, StorageError> {
        if shards.len() != self.config.total_shards() {
            return Err(StorageError::ErasureCoding(format!(
                "Expected {} shards, got {}",
                self.config.total_shards(),
                shards.len()
            )));
        }

        // Count available shards
        let available = shards.iter().filter(|s| s.is_some()).count();
        if available < self.config.data_shards {
            return Err(StorageError::InsufficientFragments {
                have: available,
                need: self.config.data_shards,
            });
        }

        // Determine shard size from first available shard
        let shard_size = shards
            .iter()
            .find_map(|s| s.as_ref().map(|s| s.data.len()))
            .ok_or_else(|| StorageError::ErasureCoding("No shards available".into()))?;

        // Prepare shards for reconstruction
        let mut shard_data: Vec<Option<Vec<u8>>> = shards
            .into_iter()
            .map(|opt| opt.map(|s| s.data))
            .collect();

        // Reconstruct missing shards
        self.rs
            .reconstruct(&mut shard_data)
            .map_err(|e| StorageError::ErasureCoding(e.to_string()))?;

        // Combine data shards
        let mut result = Vec::with_capacity(original_size);
        for i in 0..self.config.data_shards {
            if let Some(ref shard) = shard_data[i] {
                result.extend_from_slice(shard);
            } else {
                return Err(StorageError::ErasureCoding("Reconstruction failed".into()));
            }
        }

        // Trim to original size
        result.truncate(original_size);

        Ok(result)
    }

    /// Verify shard integrity
    pub fn verify(&self, shards: &[Vec<u8>]) -> Result<bool, StorageError> {
        self.rs
            .verify(shards)
            .map_err(|e| StorageError::ErasureCoding(e.to_string()))
    }
}

/// A single shard of encoded data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shard {
    /// Shard index (0 to total_shards - 1)
    pub index: usize,

    /// Shard data
    pub data: Vec<u8>,

    /// Is this a parity shard?
    pub is_parity: bool,

    /// Original shard size
    pub original_size: usize,
}

impl Shard {
    /// Get a unique ID for this shard (based on content hash + index)
    pub fn id(&self, file_hash: &str) -> String {
        format!("{}-shard-{:02}", file_hash, self.index)
    }
}

/// Encoded file with all shards and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedFile {
    /// Original file hash
    pub file_hash: String,

    /// Original file size
    pub original_size: usize,

    /// Erasure config used
    pub config: ErasureConfig,

    /// All shards
    pub shards: Vec<Shard>,
}

impl EncodedFile {
    /// Get shard IDs for DHT lookup
    pub fn shard_ids(&self) -> Vec<String> {
        self.shards
            .iter()
            .map(|s| s.id(&self.file_hash))
            .collect()
    }

    /// Calculate total encoded size
    pub fn total_size(&self) -> usize {
        self.shards.iter().map(|s| s.data.len()).sum()
    }

    /// Calculate overhead
    pub fn overhead(&self) -> f32 {
        self.total_size() as f32 / self.original_size as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erasure_config() {
        let config = ErasureConfig::default();

        assert_eq!(config.data_shards, 10);
        assert_eq!(config.parity_shards, 4);
        assert_eq!(config.total_shards(), 14);
        assert_eq!(config.min_shards(), 10);
        assert_eq!(config.max_losses(), 4);
    }

    #[test]
    fn test_encode_decode_no_loss() {
        let config = ErasureConfig::new(4, 2); // 4 data + 2 parity
        let encoder = ErasureEncoder::new(config).unwrap();
        let decoder = ErasureDecoder::new(config).unwrap();

        let original = b"Hello, CloudP2P! This is test data for erasure coding.";
        let shards = encoder.encode(original).unwrap();

        assert_eq!(shards.len(), 6);
        assert_eq!(shards.iter().filter(|s| s.is_parity).count(), 2);

        // Decode with all shards
        let shard_opts: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();
        let decoded = decoder.decode(shard_opts, original.len()).unwrap();

        assert_eq!(decoded, original.to_vec());
    }

    #[test]
    fn test_encode_decode_with_loss() {
        let config = ErasureConfig::new(4, 2); // Can lose up to 2 shards
        let encoder = ErasureEncoder::new(config).unwrap();
        let decoder = ErasureDecoder::new(config).unwrap();

        let original = b"Hello, CloudP2P! This is test data for erasure coding.";
        let shards = encoder.encode(original).unwrap();

        // Simulate losing 2 shards (indices 1 and 3)
        let mut shard_opts: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();
        shard_opts[1] = None;
        shard_opts[3] = None;

        let decoded = decoder.decode(shard_opts, original.len()).unwrap();

        assert_eq!(decoded, original.to_vec());
    }

    #[test]
    fn test_too_many_losses() {
        let config = ErasureConfig::new(4, 2); // Can only lose 2 shards
        let encoder = ErasureEncoder::new(config).unwrap();
        let decoder = ErasureDecoder::new(config).unwrap();

        let original = b"Hello, CloudP2P!";
        let shards = encoder.encode(original).unwrap();

        // Simulate losing 3 shards (more than parity allows)
        let mut shard_opts: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();
        shard_opts[0] = None;
        shard_opts[2] = None;
        shard_opts[4] = None;

        let result = decoder.decode(shard_opts, original.len());

        assert!(result.is_err());
    }

    #[test]
    fn test_large_data() {
        let config = ErasureConfig::default(); // 10 + 4
        let encoder = ErasureEncoder::new(config).unwrap();
        let decoder = ErasureDecoder::new(config).unwrap();

        // 1 MB of random data
        let original: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
        let shards = encoder.encode(&original).unwrap();

        // Lose 4 shards (maximum allowed)
        let mut shard_opts: Vec<Option<Shard>> = shards.into_iter().map(Some).collect();
        shard_opts[0] = None;
        shard_opts[5] = None;
        shard_opts[10] = None;
        shard_opts[13] = None;

        let decoded = decoder.decode(shard_opts, original.len()).unwrap();

        assert_eq!(decoded, original);
    }
}
