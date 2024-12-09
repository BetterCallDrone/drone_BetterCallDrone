#![allow(unused)]

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use toml;
use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use std::{fs, thread};

use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::controller::DroneEvent::{PacketDropped, PacketSent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, FloodRequest, Fragment, Nack, NackType, NodeType, Packet, PacketType};

pub struct BetterCallDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,

    received_flood_ids: Vec<u64>,
}

impl Drone for BetterCallDrone {
    fn new(
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Self {
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
            received_flood_ids: Vec::new(),
        }
    }

    fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        if let DroneCommand::Crash = command {
                            println!("drone {} crashed", self.id);
                            break;
                        }
                        self.handle_command(command);
                    }
                }
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.handle_packet(packet);
                    }
                },
            }
        }
    }
}

impl BetterCallDrone {
    fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::Nack(_) | PacketType::Ack(_) => self.forward_packet(packet, 0),
            PacketType::MsgFragment(_fragment) => self.handle_fragment(packet.routing_header, packet.session_id, _fragment),
            PacketType::FloodRequest(_flood_request) => self.handle_ndp(_flood_request, packet.session_id),
            PacketType::FloodResponse(_) => self.forward_packet(packet, 0),
        }
    }
    fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(_node_id, _sender) => self.add_sender(_node_id, _sender),
            DroneCommand::SetPacketDropRate(_pdr) => self.set_pdr(_pdr),
            DroneCommand::Crash => unreachable!(),
            DroneCommand::RemoveSender(_node_id) => self.remove_sender(_node_id),
        }
    }

    /// ======================================================================
    /// HANDLE PACKETS
    /// ======================================================================

    fn forward_packet(&mut self, mut packet: Packet, fragment_index: u64) {
        if self.id == packet.routing_header.hops[packet.routing_header.hop_index] {
            packet.routing_header.hop_index += 1;
            if let Some(next_hop) = packet.routing_header.hops.get(packet.routing_header.hop_index) {
                if let Some(sender) = self.packet_send.get(next_hop) {
                    println!("Packet #{} sent by drone {}", packet.session_id, self.id);
                    sender.send(packet.clone()).unwrap();                    // forwarding packet to next_hop
                    self.controller_send.send(PacketSent(packet)).unwrap();  // sending confirmation to the SC
                } else {
                    self.send_nack(packet.routing_header.clone(), fragment_index, packet.session_id, NackType::ErrorInRouting(*next_hop));
                }
            } else {
                self.send_nack(packet.routing_header.clone(), fragment_index, packet.session_id, NackType::DestinationIsDrone);
            }
        } else {
            self.send_nack(packet.routing_header.clone(), fragment_index, packet.session_id, NackType::UnexpectedRecipient(self.id));
        }
    }

    fn handle_fragment(&mut self, routing_header : SourceRoutingHeader, session_id : u64, fragment: Fragment) {
        let rh = routing_header.clone();
        let packet = Packet {
            routing_header: rh,
            session_id,
            pack_type: PacketType::MsgFragment(fragment.clone()),
        };
        if !self.should_drop_packet() {
            let index = fragment.fragment_index;
            self.forward_packet(packet, index);
        } else {
            self.send_nack(routing_header, fragment.fragment_index, session_id, NackType::Dropped);
            self.controller_send.send(PacketDropped(packet)).unwrap()       // sending confirmation of drop to SC
        }
    }

    fn should_drop_packet(&self) -> bool {
        random::<f32>() <= self.pdr
    }

    fn handle_ndp(&mut self, mut flood_request: FloodRequest, session_id: u64) {
        if self.received_flood_ids.contains(&flood_request.flood_id) {
            self.forward_flood_response(&mut flood_request, session_id);
            return;
        }
        self.received_flood_ids.push(flood_request.flood_id);
        let prev_node = flood_request.path_trace.last().unwrap().0;
        flood_request.increment(self.id, NodeType::Drone);
        let mut neighbor_found = false;
        for (n_id, n_send) in &self.packet_send {
            if n_id != &prev_node {
                neighbor_found = true;
                let packet = Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request.clone());
                n_send.send(packet).unwrap();
            }
        }
        if !neighbor_found {
            self.forward_flood_response(&mut flood_request, session_id);
        }
    }


    fn forward_flood_response(&mut self, flood_request: &mut FloodRequest, session_id: u64) {
        let mut packet = flood_request.generate_response(session_id);
        packet.routing_header.hop_index += 1;
        self.forward_packet(packet, 0);
    }

    fn send_nack(&mut self, routing_header: SourceRoutingHeader, fragment_index: u64, session_id: u64, nack_type: NackType) {
        let nack = Nack { fragment_index, nack_type };
        let self_index = routing_header
            .hops
            .iter()
            .position(|&hop| hop == self.id as u8)
            .unwrap_or(0);
        let reversed_hops: Vec<NodeId> = routing_header.hops[..=self_index]
            .iter()
            .cloned()
            .rev()
            .collect();
        if let Some(sender) = self.packet_send.get(&reversed_hops[1]) {
            println!("Nack #{} sent by drone {}: {}", session_id, self.id, nack);
            sender.send(Packet {
                pack_type: PacketType::Nack(nack),
                routing_header: SourceRoutingHeader {
                    hop_index: 1,
                    hops: reversed_hops,
                },
                session_id,
            }).unwrap();
        }
    }

    /// ======================================================================
    /// HANDLE SIMULATION CONTROLLER COMMANDS
    /// ======================================================================

    fn add_sender(&mut self, node_id : NodeId, sender: Sender<Packet>) {
        self.packet_send.insert(node_id, sender);
        println!("Added sender id: {}, to drone #{}", node_id, self.id);
    }

    fn set_pdr(&mut self, pdr: f32) {
        self.pdr = pdr;
        println!("Updated packet drop rate of drone #{} to: {}", self.id, pdr);
    }

    fn remove_sender(&mut self, node_id : NodeId) {
        if self.packet_send.remove(&node_id).is_some() {
            println!("Removed sender id: {}, from drone #{}", node_id, self.id);
        } else {
            println!("Error while trying to remove sender id: {}, from drone #{}\nSender id don't exists!", node_id, self.id);
        }
    }


}

struct SimulationController {
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    node_event_recv: Receiver<DroneEvent>,
}

impl SimulationController {
    fn crash_all(&mut self) {
        for (_, sender) in self.drones.iter() {
            sender.send(DroneCommand::Crash).unwrap();
        }
    }
}

fn main() {
    // TO IMPLEMENT LATER
}