use natural_derive::{Add, Sub};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// EUR cent. Defaults to 0€.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Add, Sub, Serialize, Deserialize,
)]
pub struct EuroCent(u64);

impl Display for EuroCent {
    /// Format [EuroCent] as 123.05€.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let eur = self.0 / 100;
        let cent = self.0 % 100;
        write!(f, "{eur}.{cent:02}€")
    }
}

impl From<u64> for EuroCent {
    fn from(value: u64) -> Self {
        EuroCent(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euro_cent_display() {
        let _ec: EuroCent = 42.into();
        assert_eq!(EuroCent::from(42).to_string(), "0.42€");
        assert_eq!(EuroCent::from(142).to_string(), "1.42€");
        assert_eq!(EuroCent(66642).to_string(), "666.42€");
        assert_eq!(EuroCent(66607).to_string(), "666.07€");
    }
}
