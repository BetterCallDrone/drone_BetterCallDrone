#[warn(non_snake_case)]
use wg_2024::tests::{generic_fragment_forward, generic_fragment_drop, generic_chain_fragment_drop};
use drone_bettercalldrone::drone::BetterCallDrone;

#[test]
fn test_generic_fragment_forward() {
    println!("testing 1");
    generic_fragment_forward::<BetterCallDrone>();
}

#[test]
fn test_generic_fragment_drop() {
    println!("testing 2");
    generic_fragment_drop::<BetterCallDrone>();
}