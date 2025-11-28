//! Storage Module - File management with erasure coding
//!
//! Handles file encryption, fragmentation, and erasure coding for redundancy.

mod erasure;
mod file_manager;
mod quota;

pub use erasure::{ErasureEncoder, ErasureDecoder, ErasureConfig};
pub use file_manager::{FileManager, FileMetadata, UploadProgress, DownloadProgress};
pub use quota::{QuotaManager, QuotaConfig, UserQuota, QuotaCheckResult, QuotaSummary, NetworkStats};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Erasure coding error: {0}")]
    ErasureCoding(String),

    #[error("Not enough fragments available: have {have}, need {need}")]
    InsufficientFragments { have: usize, need: usize },

    #[error("Fragment integrity check failed")]
    IntegrityCheckFailed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),
}
