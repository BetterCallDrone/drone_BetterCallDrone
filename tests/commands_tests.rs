#[cfg(test)]
mod commands_tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use crossbeam_channel::{unbounded};
    use wg_2024::controller::{DroneCommand, DroneEvent};
    use wg_2024::drone::Drone;
    use wg_2024::network::SourceRoutingHeader;
    use wg_2024::packet::{Fragment, Nack, NackType, Packet, PacketType};
    use drone_bettercalldrone::BetterCallDrone;

    const TIMEOUT: Duration = Duration::from_millis(400);
    #[test]
    fn test_set_pdr() {
        let (d_send, d_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded::<Packet>();
        let (d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send,
            d_command_recv,
            d_recv.clone(),
            HashMap::from([(12, d2_send.clone())]),
            1.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let mut msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 12, 21],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_command_send.send(DroneCommand::SetPacketDropRate(0.0)).unwrap();

        d_send.send(msg.clone()).unwrap();
        msg.routing_header.hop_index = 2;

        assert_eq!(d2_recv.recv_timeout(TIMEOUT).unwrap(), msg);
        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::PacketSent(msg)
        );
    }

    #[test]
    fn test_set_invalid_pdr() {
        let (c_send, c_recv) = unbounded();
        let (d1_send, d1_recv) = unbounded();
        let (d2_send, _d2_recv) = unbounded();
        let (d_command_send, d_command_recv) = unbounded();
        let (d_event_send, _d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d1_recv,
            HashMap::from([(12, d2_send.clone()), (1, c_send.clone())]),
            1.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 21],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_command_send.send(DroneCommand::SetPacketDropRate(2.)).unwrap();
        d1_send.send(msg.clone()).unwrap();

        assert_eq!(
            c_recv.recv_timeout(TIMEOUT).unwrap(),
            Packet {
                pack_type: PacketType::Nack(Nack {
                    fragment_index: 1,
                    nack_type: NackType::Dropped,
                }),
                routing_header: SourceRoutingHeader {
                    hop_index: 1,
                    hops: vec![11, 1],
                },
                session_id: 1,
            }
        );
    }

    #[test]
    fn test_add_sender() {
        let (d_send, d_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded::<Packet>();
        let (d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send,
            d_command_recv,
            d_recv.clone(),
            HashMap::new(),
            0.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let mut msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 12, 21],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_command_send.send(DroneCommand::AddSender(12, d2_send)).unwrap();

        d_send.send(msg.clone()).unwrap();
        msg.routing_header.hop_index = 2;

        assert_eq!(d2_recv.recv_timeout(TIMEOUT).unwrap(), msg);
        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::PacketSent(msg)
        );
    }

    #[test]
    fn test_add_existing_sender() {
        let (d_send, d_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded::<Packet>();
        let (d_command_send, d_command_recv) = unbounded();
        let (d_event_send, d_event_recv) = unbounded();

        let mut drone = BetterCallDrone::new(
            11,
            d_event_send,
            d_command_recv,
            d_recv.clone(),
            HashMap::from([(12, d2_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone.run();
        });

        let mut msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 12, 21],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d_command_send.send(DroneCommand::AddSender(12, d2_send)).unwrap();

        d_send.send(msg.clone()).unwrap();
        msg.routing_header.hop_index = 2;

        assert_eq!(d2_recv.recv_timeout(TIMEOUT).unwrap(), msg);
        assert_eq!(
            d_event_recv.recv_timeout(TIMEOUT).unwrap(),
            DroneEvent::PacketSent(msg)
        );
    }

    #[test]
    fn test_remove_sender() {
        let (c_send, c_recv) = unbounded();
        let (d1_send, d1_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded();
        let (d3_send, _d3_recv) = unbounded();
        let (_d_command_send, d_command_recv) = unbounded();
        let (d2_command_send, d2_command_recv) = unbounded();
        let (d_event_send, _d_event_recv) = unbounded();

        let mut drone1 = BetterCallDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d1_recv,
            HashMap::from([(12, d2_send.clone()), (1, c_send.clone())]),
            0.0,
        );

        let mut drone2 = BetterCallDrone::new(
            12,
            d_event_send.clone(),
            d2_command_recv.clone(),
            d2_recv,
            HashMap::from([(11, d1_send.clone()), (13, d3_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone1.run();
        });

        thread::spawn(move || {
            drone2.run();
        });

        let msg = Packet::new_fragment(
            SourceRoutingHeader {
                hop_index: 1,
                hops: vec![1, 11, 12, 13, 21],
            },
            1,
            Fragment {
                fragment_index: 1,
                total_n_fragments: 1,
                length: 128,
                data: [1; 128],
            },
        );

        d2_command_send.send(DroneCommand::RemoveSender(13)).unwrap();
        d1_send.send(msg.clone()).unwrap();

        assert_eq!(
            c_recv.recv_timeout(TIMEOUT).unwrap(),
            Packet {
                pack_type: PacketType::Nack(Nack {
                    fragment_index: 1,
                    nack_type: NackType::ErrorInRouting(13),
                }),
                routing_header: SourceRoutingHeader {
                    hop_index: 2,
                    hops: vec![12, 11, 1],
                },
                session_id: 1,
            }
        );
    }

    #[test]
    fn test_remove_non_existing_sender() {
        let (_d_send, d_recv) = unbounded();
        let (d2_send, _d2_recv) = unbounded::<Packet>();
        let (d_command_send, d_command_recv) = unbounded();
        let (d_event_send, _d_event_recv) = unbounded();

        let drone = Arc::new(Mutex::new(BetterCallDrone::new(
            11,
            d_event_send,
            d_command_recv,
            d_recv.clone(),
            HashMap::from([(12, d2_send.clone())]),
            1.0,
        )));

        let drone_clone = Arc::clone(&drone);
        thread::spawn(move || {
            drone_clone.lock().unwrap().run();
        });

        d_command_send.send(DroneCommand::RemoveSender(14)).unwrap();

        let drone = drone.lock().unwrap();
        assert_eq!(drone.packet_send.len(), 1);
        assert!(drone.packet_send.get(&14).is_none());
    }
}