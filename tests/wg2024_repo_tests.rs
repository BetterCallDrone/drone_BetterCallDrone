#[cfg(test)]
mod wg2024_repo_tests {
    use wg_2024::tests::{generic_fragment_forward, generic_fragment_drop, generic_chain_fragment_drop, generic_chain_fragment_ack};
    use drone_bettercalldrone::BetterCallDrone;

    #[test]
    fn test_generic_fragment_forward() {
        generic_fragment_forward::<BetterCallDrone>();
    }

    #[test]
    fn test_generic_fragment_drop() {
        generic_fragment_drop::<BetterCallDrone>();
    }

    #[test]
    fn test_generic_chain_fragment_drop() {
        generic_chain_fragment_drop::<BetterCallDrone>();
    }

    #[test]
    fn test_generic_chain_fragment_ack() {
        generic_chain_fragment_ack::<BetterCallDrone>();
    }
}