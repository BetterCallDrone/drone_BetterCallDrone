use wg_2024::tests::{generic_fragment_forward, generic_fragment_drop};
use drone_BetterCallDrone::drone::BetterCallDrone;

#[test]
fn test_forwarding() {
    println!("testing 1");
    generic_fragment_forward::<BetterCallDrone>();
}

#[test]
fn test_packet_drop() {
    println!("testing 2");
    generic_fragment_drop::<BetterCallDrone>();
}
