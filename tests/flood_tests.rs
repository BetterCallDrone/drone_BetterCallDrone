#[cfg(test)]
mod flood_tests {
    use std::collections::HashMap;
    use std::thread;
    use crossbeam_channel::unbounded;
    use wg_2024::drone::Drone;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{FloodRequest, NodeType, Packet, PacketType};
    use drone_bettercalldrone::BetterCallDrone;

    fn create_msg(id: NodeId, fid: u64) -> Packet {
        Packet {
            pack_type: PacketType::FloodRequest(FloodRequest {
                flood_id: fid,
                initiator_id: id,
                path_trace: vec![(id, NodeType::Client)],
            }),
            routing_header: SourceRoutingHeader {
                hop_index: 0,
                hops: vec![],
            },
            session_id: 4,
        }
    }

    #[test]
    fn test_flood_straight() {
        let (c_send, c_recv) = unbounded();
        let (d1_send, d1_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded();
        let (d3_send, d3_recv) = unbounded();
        let (s_send, s_recv) = unbounded();
        let (_d_comm_send, d_comm_recv) = unbounded();
        let (d_events_send, _d_events_recv) = unbounded();

        let mut drone1 = BetterCallDrone::new(
            11,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d1_recv,
            HashMap::from([(1, c_send.clone()), (12, d2_send.clone())]),
            0.0,
        );

        let mut drone2 = BetterCallDrone::new(
            12,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d2_recv,
            HashMap::from([(11, d1_send.clone()), (13, d3_send.clone())]),
            0.0,
        );

        let mut drone3 = BetterCallDrone::new(
            13,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d3_recv,
            HashMap::from([(12, d2_send.clone()), (21, s_send.clone())]),
            0.0,
        );

        let mut server = BetterCallDrone::new(
            21,
            d_events_send.clone(),
            d_comm_recv.clone(),
            s_recv,
            HashMap::from([(13, d3_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone1.run();
        });
        thread::spawn(move || {
            drone2.run();
        });
        thread::spawn(move || {
            drone3.run();
        });
        thread::spawn(move || {
            server.run();
        });

        let msg = create_msg(1, 777);

        d1_send.send(msg).unwrap();

        if let Ok(packet) = c_recv.recv_timeout(std::time::Duration::from_secs(1)) {
            match packet.pack_type {
                PacketType::FloodResponse(response) => {
                    assert_eq!(response.flood_id, 777);
                    assert_eq!(response.path_trace, vec![(1, NodeType::Client), (11, NodeType::Drone), (12, NodeType::Drone), (13, NodeType::Drone), (21, NodeType::Drone)]);
                }
                _ => panic!("Unexpected packet: {:?}", packet.pack_type),
            }
        } else {
            panic!("Timeout: no packet received");
        }
    }

    #[test]
    fn test_flood_2branch() {
        let (c_send, c_recv) = unbounded();
        let (d1_send, d1_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded();
        let (d3_send, d3_recv) = unbounded();
        let (d4_send, d4_recv) = unbounded();
        let (s_send, s_recv) = unbounded();
        let (_d_comm_send, d_comm_recv) = unbounded();
        let (d_events_send, _d_events_recv) = unbounded();

        let mut drone1 = BetterCallDrone::new(
            11,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d1_recv,
            HashMap::from([(1, c_send.clone()), (12, d2_send.clone())]),
            0.0,
        );

        let mut drone2 = BetterCallDrone::new(
            12,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d2_recv,
            HashMap::from([(11, d1_send.clone()), (13, d3_send.clone()), (14, d4_send.clone())]),
            0.0,
        );

        let mut drone3 = BetterCallDrone::new(
            13,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d3_recv,
            HashMap::from([(12, d2_send.clone()), (21, s_send.clone())]),
            0.0,
        );

        let mut drone4 = BetterCallDrone::new(
            14,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d4_recv,
            HashMap::from([(12, d2_send.clone()), (21, s_send.clone())]),
            0.0,
        );

        let mut server = BetterCallDrone::new(
            21,
            d_events_send.clone(),
            d_comm_recv.clone(),
            s_recv,
            HashMap::from([(13, d3_send.clone()), (14, d4_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone1.run();
        });
        thread::spawn(move || {
            drone2.run();
        });
        thread::spawn(move || {
            drone3.run();
        });
        thread::spawn(move || {
            drone4.run();
        });
        thread::spawn(move || {
            server.run();
        });

        let msg = create_msg(1, 777);

        d1_send.send(msg).unwrap();

        for _ in 0..2 {
            if let Ok(packet) = c_recv.recv_timeout(std::time::Duration::from_secs(1)) {
                match packet.pack_type {
                    PacketType::FloodResponse(response) => {
                        assert_eq!(response.flood_id, 777);
                    }
                    _ => panic!("Unexpected packet: {:?}", packet.pack_type),
                }
            } else {
                panic!("Timeout: no packet received");
            }
        }
    }

    #[test]
    fn test_flood_double_chain() {
        let (c1_send, c1_recv) = unbounded();
        let (c2_send, c2_recv) = unbounded();
        let (d1_send, d1_recv) = unbounded();
        let (d2_send, d2_recv) = unbounded();
        let (d3_send, d3_recv) = unbounded();
        let (d4_send, d4_recv) = unbounded();
        let (d5_send, d5_recv) = unbounded();
        let (d6_send, d6_recv) = unbounded();
        let (s1_send, s1_recv) = unbounded();
        let (s2_send, s2_recv) = unbounded();
        let (_d_comm_send, d_comm_recv) = unbounded();
        let (d_events_send, _d_events_recv) = unbounded();

        let mut drone1 = BetterCallDrone::new(
            11,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d1_recv,
            HashMap::from([(1, c1_send.clone()), (12, d2_send.clone()), (14, d2_send.clone())]),
            0.0,
        );

        let mut drone2 = BetterCallDrone::new(
            12,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d2_recv,
            HashMap::from([(11, d1_send.clone()), (13, d3_send.clone()), (15, d5_send.clone())]),
            0.0,
        );

        let mut drone3 = BetterCallDrone::new(
            13,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d3_recv,
            HashMap::from([(12, d2_send.clone()), (21, s1_send.clone()), (16, d6_send.clone())]),
            0.0,
        );

        let mut drone4 = BetterCallDrone::new(
            14,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d4_recv,
            HashMap::from([(2, c2_send.clone()), (15, d5_send.clone()), (11, d1_send.clone())]),
            0.0,
        );

        let mut drone5 = BetterCallDrone::new(
            15,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d5_recv,
            HashMap::from([(14, d4_send.clone()), (16, d6_send.clone()), (12, d2_send.clone())]),
            0.0,
        );

        let mut drone6 = BetterCallDrone::new(
            16,
            d_events_send.clone(),
            d_comm_recv.clone(),
            d6_recv,
            HashMap::from([(15, d5_send.clone()), (13, d3_send.clone()), (22, s2_send.clone())]),
            0.0,
        );

        let mut server1 = BetterCallDrone::new(
            21,
            d_events_send.clone(),
            d_comm_recv.clone(),
            s1_recv,
            HashMap::from([(13, d3_send.clone())]),
            0.0,
        );

        let mut server2 = BetterCallDrone::new(
            22,
            d_events_send.clone(),
            d_comm_recv.clone(),
            s2_recv,
            HashMap::from([(16, d3_send.clone())]),
            0.0,
        );

        thread::spawn(move || {
            drone1.run();
        });
        thread::spawn(move || {
            drone2.run();
        });
        thread::spawn(move || {
            drone3.run();
        });
        thread::spawn(move || {
            drone4.run();
        });
        thread::spawn(move || {
            drone5.run();
        });
        thread::spawn(move || {
            drone6.run();
        });
        thread::spawn(move || {
            server1.run();
        });
        thread::spawn(move || {
            server2.run();
        });

        let msg1 = create_msg(1, 777);
        let msg2 = create_msg(2, 555);

        d1_send.send(msg1).unwrap();
        d4_send.send(msg2).unwrap();

        let mut c1_vec = Vec::new();
        loop {
            match c1_recv.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(packet) => {
                match packet.pack_type {
                    PacketType::FloodResponse(response) => {
                        c1_vec.push(response.flood_id);
                    },
                    PacketType::FloodRequest(request) => {
                        let packet = request.generate_response(4);
                        d1_send.send(packet).unwrap();
                    },
                    _ => panic!("Unexpected packet: {:?}", packet.pack_type),
                }
                }
                Err(_) => break,
            }
        }

        let mut c2_vec = Vec::new();
        loop {
            match c2_recv.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(packet) => {
                    match packet.pack_type {
                        PacketType::FloodResponse(response) => {
                            c2_vec.push(response.flood_id);
                        },
                        PacketType::FloodRequest(request) => {
                            let packet = request.generate_response(4);
                            d4_send.send(packet).unwrap();
                        },
                        _ => panic!("Unexpected packet: {:?}", packet.pack_type),
                    }
                }
                Err(_) => break,
            }
        }

        assert!(c1_vec.iter().all(|&flood_id| flood_id == 777));
        assert!(c2_vec.iter().all(|&flood_id| flood_id == 555));
    }
}