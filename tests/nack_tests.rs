#[cfg(test)]
mod nack_tests {
    use std::collections::HashMap;
    use std::thread;
    use std::time::Duration;
    use crossbeam_channel::unbounded;
    use wg_2024::controller::DroneEvent;
    use wg_2024::drone::Drone;
    use wg_2024::network::SourceRoutingHeader;
    use wg_2024::packet::{FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
    use drone_bettercalldrone::BetterCallDrone;

    const TIMEOUT: Duration = Duration::from_millis(400);

    #[test]
    fn test_unexpected_recipient(){
        let (c_send, c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, _d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 16, 21],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_send.send(msg).unwrap();

        let nack = Packet {
            pack_type: PacketType::Nack(Nack {
                fragment_index: 1,
                nack_type: NackType::UnexpectedRecipient(11),
            }),
            routing_header: SourceRoutingHeader {
                hop_index: 1,
                hops: vec![11, 1],
            },
            session_id: 1,
        };

        assert_eq!(c_recv.recv_timeout(TIMEOUT).unwrap(), nack);
    }

    #[test]
    fn test_destination_is_drone(){
        let (c_send, c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, _d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_send.send(msg).unwrap();

        let nack = Packet {
            pack_type: PacketType::Nack(Nack {
                fragment_index: 1,
                nack_type: NackType::DestinationIsDrone,
            }),
            routing_header: SourceRoutingHeader {
                hop_index: 1,
                hops: vec![11, 1],
            },
            session_id: 1,
        };

        assert_eq!(c_recv.recv_timeout(TIMEOUT).unwrap(), nack);
    }

    #[test]
    fn test_error_in_routing(){
        let (c_send, c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, _d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 12],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_send.send(msg).unwrap();

        let nack = Packet {
            pack_type: PacketType::Nack(Nack {
                fragment_index: 1,
                nack_type: NackType::ErrorInRouting(12),
            }),
            routing_header: SourceRoutingHeader {
                hop_index: 1,
                hops: vec![11, 1],
            },
            session_id: 1,
        };

        assert_eq!(c_recv.recv_timeout(TIMEOUT).unwrap(), nack);
    }

    #[test]
    fn test_dropped(){
        let (c_send, c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (d2_send, _d2_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone()), (12, d2_send)]),
            1.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 12],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_send.send(msg.clone()).unwrap();

        let nack = Packet::new_nack(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![11, 1],
            },
            1,
            Nack {
                fragment_index: 1,
                nack_type: NackType::Dropped,
            },
        );

        assert_eq!(c_recv.recv_timeout(TIMEOUT).unwrap(), nack);
        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::PacketDropped(msg)
        );
    }

    #[test]
    fn test_nack_to_sc(){
        let (c_send, _c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (d2_send, _d2_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone()), (12, d2_send)]),
            1.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let nack = Packet::new_nack(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 12, 13],
            },
            1,
            Nack {
                fragment_index: 1,
                nack_type: NackType::Dropped,
            },
        );

        d_send.send(nack.clone()).unwrap();

        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::ControllerShortcut(nack.clone())
        );
    }

    #[test]
    fn test_ack_to_sc(){
        let (c_send, _c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (d2_send, _d2_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone()), (12, d2_send)]),
            1.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let ack = Packet::new_ack(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 12, 13],
            },
            1,
            1,
        );

        d_send.send(ack.clone()).unwrap();

        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::ControllerShortcut(ack.clone())
        );
    }

    #[test]
    fn test_flood_response_to_sc(){
        let (c_send, _c_recv) = unbounded();
        let (d_send, d_recv) = unbounded();
        let (d2_send, _d2_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv,
            HashMap::from([(1, c_send.clone()), (12, d2_send)]),
            1.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let ack = Packet::new_flood_response(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 12, 13],
            },
            1,
            FloodResponse { flood_id: 777, path_trace: vec![(1, NodeType::Client), (11, NodeType::Drone), (14, NodeType::Drone)] },
        );

        d_send.send(ack.clone()).unwrap();

        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::ControllerShortcut(ack.clone())
        );
    }
}