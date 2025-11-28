//! Quota Management System
//!
//! Enforces fair storage usage: users must contribute storage equal to what they use.
//! This ensures the P2P network remains balanced and sustainable.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Storage quota configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaConfig {
    /// Minimum storage contribution required (bytes)
    pub min_contribution: u64,
    /// Maximum storage a user can use (bytes)
    pub max_usage: u64,
    /// Ratio of contribution to usage (1.0 = equal, 1.5 = contribute 50% more)
    pub contribution_ratio: f64,
    /// Grace period for new users (seconds)
    pub grace_period_secs: u64,
    /// Path to store quota data
    pub data_path: PathBuf,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            min_contribution: 1024 * 1024 * 100, // 100 MB minimum
            max_usage: 1024 * 1024 * 1024 * 100, // 100 GB maximum
            contribution_ratio: 1.0,              // 1:1 ratio
            grace_period_secs: 7 * 24 * 60 * 60,  // 7 days grace period
            data_path: PathBuf::from("./libredrive_data"),
        }
    }
}

/// User's storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuota {
    /// User's public ID
    pub user_id: String,
    /// Total bytes used by this user
    pub bytes_used: u64,
    /// Total bytes this user is contributing to the network
    pub bytes_contributed: u64,
    /// Number of files stored
    pub files_count: u64,
    /// Number of shards hosted for other users
    pub shards_hosted: u64,
    /// Timestamp when user joined
    pub joined_at: u64,
    /// Last activity timestamp
    pub last_active: u64,
    /// Is user in grace period?
    pub in_grace_period: bool,
}

impl UserQuota {
    pub fn new(user_id: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            user_id,
            bytes_used: 0,
            bytes_contributed: 0,
            files_count: 0,
            shards_hosted: 0,
            joined_at: now,
            last_active: now,
            in_grace_period: true,
        }
    }

    /// Calculate available storage based on contribution
    pub fn available_storage(&self, config: &QuotaConfig) -> u64 {
        if self.in_grace_period {
            // During grace period, allow some free storage
            return config.min_contribution;
        }

        let allowed = (self.bytes_contributed as f64 / config.contribution_ratio) as u64;
        allowed.min(config.max_usage).saturating_sub(self.bytes_used)
    }

    /// Check if user can upload more data
    pub fn can_upload(&self, size: u64, config: &QuotaConfig) -> bool {
        let available = self.available_storage(config);
        available >= size
    }

    /// Get quota status as percentage (0-100)
    pub fn usage_percentage(&self, config: &QuotaConfig) -> f64 {
        if self.bytes_contributed == 0 && !self.in_grace_period {
            return 100.0; // No contribution = full quota
        }

        let max_allowed = if self.in_grace_period {
            config.min_contribution
        } else {
            (self.bytes_contributed as f64 / config.contribution_ratio) as u64
        };

        if max_allowed == 0 {
            return 100.0;
        }

        (self.bytes_used as f64 / max_allowed as f64 * 100.0).min(100.0)
    }

    /// Check if grace period has expired
    pub fn check_grace_period(&mut self, config: &QuotaConfig) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if self.in_grace_period && (now - self.joined_at) > config.grace_period_secs {
            self.in_grace_period = false;
        }
    }
}

/// Quota Manager - handles all quota operations
pub struct QuotaManager {
    config: QuotaConfig,
    quotas: HashMap<String, UserQuota>,
}

impl QuotaManager {
    pub fn new(config: QuotaConfig) -> Self {
        Self {
            config,
            quotas: HashMap::new(),
        }
    }

    /// Get or create user quota
    pub fn get_user_quota(&mut self, user_id: &str) -> &mut UserQuota {
        if !self.quotas.contains_key(user_id) {
            let quota = UserQuota::new(user_id.to_string());
            self.quotas.insert(user_id.to_string(), quota);
        }
        self.quotas.get_mut(user_id).unwrap()
    }

    /// Check if user can upload
    pub fn can_upload(&mut self, user_id: &str, size: u64) -> QuotaCheckResult {
        let quota = self.get_user_quota(user_id);
        quota.check_grace_period(&self.config);

        if quota.can_upload(size, &self.config) {
            QuotaCheckResult::Allowed
        } else {
            let needed = self.calculate_needed_contribution(user_id, size);
            QuotaCheckResult::InsufficientQuota {
                current_contribution: quota.bytes_contributed,
                needed_contribution: needed,
                message: format!(
                    "Para fazer upload de {} bytes, você precisa contribuir {} bytes para a rede.",
                    size, needed
                ),
            }
        }
    }

    /// Calculate how much contribution is needed for a given upload
    fn calculate_needed_contribution(&self, user_id: &str, additional_bytes: u64) -> u64 {
        let quota = self.quotas.get(user_id).unwrap();
        let total_needed = quota.bytes_used + additional_bytes;
        let contribution_needed = (total_needed as f64 * self.config.contribution_ratio) as u64;
        contribution_needed.saturating_sub(quota.bytes_contributed)
    }

    /// Record a file upload
    pub fn record_upload(&mut self, user_id: &str, size: u64) {
        let quota = self.get_user_quota(user_id);
        quota.bytes_used += size;
        quota.files_count += 1;
        quota.last_active = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Record a file deletion
    pub fn record_deletion(&mut self, user_id: &str, size: u64) {
        if let Some(quota) = self.quotas.get_mut(user_id) {
            quota.bytes_used = quota.bytes_used.saturating_sub(size);
            quota.files_count = quota.files_count.saturating_sub(1);
        }
    }

    /// Record hosting a shard for another user
    pub fn record_shard_hosted(&mut self, user_id: &str, shard_size: u64) {
        let quota = self.get_user_quota(user_id);
        quota.bytes_contributed += shard_size;
        quota.shards_hosted += 1;
        quota.last_active = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Record removing a hosted shard
    pub fn record_shard_removed(&mut self, user_id: &str, shard_size: u64) {
        if let Some(quota) = self.quotas.get_mut(user_id) {
            quota.bytes_contributed = quota.bytes_contributed.saturating_sub(shard_size);
            quota.shards_hosted = quota.shards_hosted.saturating_sub(1);
        }
    }

    /// Get network statistics
    pub fn get_network_stats(&self) -> NetworkStats {
        let mut total_used = 0u64;
        let mut total_contributed = 0u64;
        let mut total_users = 0u64;
        let mut active_users = 0u64;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for quota in self.quotas.values() {
            total_used += quota.bytes_used;
            total_contributed += quota.bytes_contributed;
            total_users += 1;

            // Active in last 24 hours
            if now - quota.last_active < 24 * 60 * 60 {
                active_users += 1;
            }
        }

        NetworkStats {
            total_storage_used: total_used,
            total_storage_contributed: total_contributed,
            total_users,
            active_users,
            average_contribution: if total_users > 0 {
                total_contributed / total_users
            } else {
                0
            },
        }
    }

    /// Get user's quota summary
    pub fn get_quota_summary(&mut self, user_id: &str) -> QuotaSummary {
        let quota = self.get_user_quota(user_id);
        quota.check_grace_period(&self.config);

        QuotaSummary {
            bytes_used: quota.bytes_used,
            bytes_contributed: quota.bytes_contributed,
            bytes_available: quota.available_storage(&self.config),
            usage_percentage: quota.usage_percentage(&self.config),
            files_count: quota.files_count,
            shards_hosted: quota.shards_hosted,
            in_grace_period: quota.in_grace_period,
            contribution_ratio: self.config.contribution_ratio,
        }
    }
}

/// Result of quota check
#[derive(Debug, Clone)]
pub enum QuotaCheckResult {
    Allowed,
    InsufficientQuota {
        current_contribution: u64,
        needed_contribution: u64,
        message: String,
    },
}

/// Network-wide statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub total_storage_used: u64,
    pub total_storage_contributed: u64,
    pub total_users: u64,
    pub active_users: u64,
    pub average_contribution: u64,
}

/// User quota summary for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaSummary {
    pub bytes_used: u64,
    pub bytes_contributed: u64,
    pub bytes_available: u64,
    pub usage_percentage: f64,
    pub files_count: u64,
    pub shards_hosted: u64,
    pub in_grace_period: bool,
    pub contribution_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_user_grace_period() {
        let config = QuotaConfig::default();
        let mut manager = QuotaManager::new(config.clone());

        let result = manager.can_upload("user1", 50 * 1024 * 1024); // 50 MB
        assert!(matches!(result, QuotaCheckResult::Allowed));
    }

    #[test]
    fn test_quota_enforcement() {
        let mut config = QuotaConfig::default();
        config.grace_period_secs = 0; // No grace period

        let mut manager = QuotaManager::new(config);

        // User with no contribution
        let quota = manager.get_user_quota("user1");
        quota.in_grace_period = false;

        let result = manager.can_upload("user1", 100 * 1024 * 1024);
        assert!(matches!(result, QuotaCheckResult::InsufficientQuota { .. }));

        // Add contribution
        manager.record_shard_hosted("user1", 200 * 1024 * 1024);

        let result = manager.can_upload("user1", 100 * 1024 * 1024);
        assert!(matches!(result, QuotaCheckResult::Allowed));
    }

    #[test]
    fn test_contribution_ratio() {
        let mut config = QuotaConfig::default();
        config.contribution_ratio = 1.5; // Must contribute 50% more than used
        config.grace_period_secs = 0;

        let mut manager = QuotaManager::new(config);

        // Add 150 MB contribution
        manager.record_shard_hosted("user1", 150 * 1024 * 1024);

        let quota = manager.get_user_quota("user1");
        quota.in_grace_period = false;

        // Should allow 100 MB upload (150 / 1.5 = 100)
        let result = manager.can_upload("user1", 100 * 1024 * 1024);
        assert!(matches!(result, QuotaCheckResult::Allowed));

        // Should NOT allow 101 MB upload
        let result = manager.can_upload("user1", 101 * 1024 * 1024);
        assert!(matches!(result, QuotaCheckResult::InsufficientQuota { .. }));
    }
}
