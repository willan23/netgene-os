//! Conflict-Free Replicated Data Types (CRDT) for state convergence across mesh nodes.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Last-Write-Wins Register (LWW-Register) CRDT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T> {
    pub value: T,
    pub timestamp: DateTime<Utc>,
    pub writer_id: String,
}

impl<T: Clone> LWWRegister<T> {
    pub fn new(value: T, writer_id: impl Into<String>) -> Self {
        Self {
            value,
            timestamp: Utc::now(),
            writer_id: writer_id.into(),
        }
    }

    pub fn with_timestamp(value: T, timestamp: DateTime<Utc>, writer_id: impl Into<String>) -> Self {
        Self {
            value,
            timestamp,
            writer_id: writer_id.into(),
        }
    }

    /// Merges another register, keeping the one with higher timestamp or lexicographical writer_id tie-break.
    pub fn merge(&mut self, other: LWWRegister<T>) -> bool {
        if other.timestamp > self.timestamp || (other.timestamp == self.timestamp && other.writer_id > self.writer_id) {
            self.value = other.value;
            self.timestamp = other.timestamp;
            self.writer_id = other.writer_id;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_lww_newer_wins() {
        let t1 = Utc::now();
        let t2 = t1 + Duration::milliseconds(100);

        let mut reg_a = LWWRegister::with_timestamp("old_value", t1, "node-A");
        let reg_b = LWWRegister::with_timestamp("new_value", t2, "node-B");

        let updated = reg_a.merge(reg_b);
        assert!(updated, "Newer register should win the merge");
        assert_eq!(reg_a.value, "new_value");
        assert_eq!(reg_a.writer_id, "node-B");
    }

    #[test]
    fn test_lww_older_does_not_overwrite() {
        let t2 = Utc::now();
        let t1 = t2 - Duration::milliseconds(200);

        let mut reg_a = LWWRegister::with_timestamp("current", t2, "node-A");
        let reg_b = LWWRegister::with_timestamp("stale", t1, "node-B");

        let updated = reg_a.merge(reg_b);
        assert!(!updated, "Older register should NOT overwrite newer");
        assert_eq!(reg_a.value, "current");
    }

    #[test]
    fn test_lww_tie_break_by_writer_id() {
        let t = Utc::now();
        let mut reg_a = LWWRegister::with_timestamp(42u32, t, "node-A");
        let reg_z = LWWRegister::with_timestamp(99u32, t, "node-Z");

        // node-Z > node-A lexicographically, so node-Z wins
        let updated = reg_a.merge(reg_z);
        assert!(updated);
        assert_eq!(reg_a.value, 99);
        assert_eq!(reg_a.writer_id, "node-Z");
    }

    #[test]
    fn test_lww_same_writer_same_time_no_update() {
        let t = Utc::now();
        let mut reg = LWWRegister::with_timestamp("original", t, "node-A");
        let duplicate = LWWRegister::with_timestamp("duplicate", t, "node-A");
        // Same timestamp, same writer_id — node-A is NOT > node-A
        let updated = reg.merge(duplicate);
        assert!(!updated);
        assert_eq!(reg.value, "original");
    }

    #[test]
    fn test_lww_new_creates_correctly() {
        let reg = LWWRegister::new("hello", "writer-1");
        assert_eq!(reg.value, "hello");
        assert_eq!(reg.writer_id, "writer-1");
    }

    #[test]
    fn test_lww_json_serialization() {
        let reg = LWWRegister::new(serde_json::json!({"status": "active"}), "node-01");
        let json = serde_json::to_string(&reg).unwrap();
        let back: LWWRegister<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value["status"], "active");
        assert_eq!(back.writer_id, "node-01");
    }

    #[test]
    fn test_lww_integer_values() {
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(1);
        let mut score_a = LWWRegister::with_timestamp(100i64, t1, "scorer-1");
        let score_b = LWWRegister::with_timestamp(200i64, t2, "scorer-2");
        score_a.merge(score_b);
        assert_eq!(score_a.value, 200i64);
    }
}
