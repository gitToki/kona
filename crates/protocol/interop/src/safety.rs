//! Message safety level for interoperability.
use alloc::string::{String, ToString};
use core::str::FromStr;
use derive_more::Display;
use thiserror::Error;
/// The safety level of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum SafetyLevel {
    /// The message is finalized.
    Finalized,
    /// The message is safe.
    Safe,
    /// The message is safe locally.
    LocalSafe,
    /// The message is unsafe across chains.
    CrossUnsafe,
    /// The message is unsafe.
    Unsafe,
    /// The message is invalid.
    Invalid,
}

impl FromStr for SafetyLevel {
    type Err = SafetyLevelParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "finalized" => Ok(Self::Finalized),
            "safe" => Ok(Self::Safe),
            "local-safe" | "localsafe" => Ok(Self::LocalSafe),
            "cross-unsafe" | "crossunsafe" => Ok(Self::CrossUnsafe),
            "unsafe" => Ok(Self::Unsafe),
            "invalid" => Ok(Self::Invalid),
            _ => Err(SafetyLevelParseError(s.to_string())),
        }
    }
}

/// Error when parsing SafetyLevel from string.
#[derive(Error, Debug)]
#[error("Invalid SafetyLevel, error: {0}")]
pub struct SafetyLevelParseError(pub String);

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;

    #[test]
    fn test_safety_level_serde() {
        let level = SafetyLevel::Finalized;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, r#""finalized""#);

        let level: SafetyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, SafetyLevel::Finalized);
    }

    #[test]
    fn test_serde_safety_level_fails() {
        let json = r#""failed""#;
        let level: Result<SafetyLevel, _> = serde_json::from_str(json);
        assert!(level.is_err());
    }
}

#[test]
fn test_safety_level_from_str_valid() {
    assert_eq!(SafetyLevel::from_str("finalized").unwrap(), SafetyLevel::Finalized);
    assert_eq!(SafetyLevel::from_str("safe").unwrap(), SafetyLevel::Safe);
    assert_eq!(SafetyLevel::from_str("local-safe").unwrap(), SafetyLevel::LocalSafe);
    assert_eq!(SafetyLevel::from_str("localsafe").unwrap(), SafetyLevel::LocalSafe);
    assert_eq!(SafetyLevel::from_str("cross-unsafe").unwrap(), SafetyLevel::CrossUnsafe);
    assert_eq!(SafetyLevel::from_str("crossunsafe").unwrap(), SafetyLevel::CrossUnsafe);
    assert_eq!(SafetyLevel::from_str("unsafe").unwrap(), SafetyLevel::Unsafe);
    assert_eq!(SafetyLevel::from_str("invalid").unwrap(), SafetyLevel::Invalid);
}

#[test]
fn test_safety_level_from_str_invalid() {
    assert!(SafetyLevel::from_str("unknown").is_err());
    assert!(SafetyLevel::from_str("123").is_err());
    assert!(SafetyLevel::from_str("").is_err());
    assert!(SafetyLevel::from_str("safe ").is_err());
}
