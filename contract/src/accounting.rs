/// Returns the exact NEAR storage charge for a measured byte increase.
pub fn storage_cost_yocto(added_bytes: u64, byte_cost_yocto: u128) -> Option<u128> {
    u128::from(added_bytes).checked_mul(byte_cost_yocto)
}

/// A storage decrease is not a valid funding measurement for creation/join.
pub fn storage_bytes_added(before: u64, after: u64) -> Option<u64> {
    after.checked_sub(before)
}

#[cfg(test)]
mod tests {
    use super::{storage_bytes_added, storage_cost_yocto};

    #[test]
    fn computes_cost_from_measured_bytes_and_protocol_price() {
        assert_eq!(
            storage_cost_yocto(125, 10_000_000_000_000_000_000),
            Some(1_250_000_000_000_000_000_000)
        );
    }

    #[test]
    fn zero_new_bytes_costs_nothing() {
        assert_eq!(storage_cost_yocto(0, 10), Some(0));
    }

    #[test]
    fn rejects_cost_overflow() {
        assert_eq!(storage_cost_yocto(u64::MAX, u128::MAX), None);
    }

    #[test]
    fn rejects_negative_storage_delta() {
        assert_eq!(storage_bytes_added(100, 99), None);
        assert_eq!(storage_bytes_added(100, 100), Some(0));
        assert_eq!(storage_bytes_added(100, 101), Some(1));
    }
}
