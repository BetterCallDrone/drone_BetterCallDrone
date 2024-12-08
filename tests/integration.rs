use wg_2024::tests::{generic_fragment_forward, generic_fragment_drop};
use drone_bettercalldrone::drone::BetterCallDrone;

#[test]
fn test_generic_fragment_forward() {
    generic_fragment_forward::<BetterCallDrone>();
}

#[test]
fn test_generic_fragment_drop() {
    generic_fragment_drop::<BetterCallDrone>();
}