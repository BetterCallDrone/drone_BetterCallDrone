#[allow(unused)]
use crossbeam_channel::{select_biased, Receiver, Sender};
use std::collections::{HashMap, HashSet};
use colored::*;
use rand::random;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::controller::DroneEvent::{PacketDropped, PacketSent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, Fragment, Nack, NackType, NodeType, Packet, PacketType};

pub struct BetterCallDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,

    received_flood_ids: HashSet<(u64, NodeId)>,
    debug: bool,
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
            received_flood_ids: HashSet::new(),
            debug: true,
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
    pub fn handle_packet(&mut self, packet: Packet) {
        self.log_received(&packet);
        match packet.pack_type {
            PacketType::Nack(_) | PacketType::Ack(_) | PacketType::FloodResponse(_) => self.forward_packet(packet, 0),
            PacketType::MsgFragment(fragment) => self.handle_fragment(packet.routing_header, packet.session_id, fragment),
            PacketType::FloodRequest(flood_request) => self.handle_ndp(flood_request, packet.session_id),
        }
    }
    pub fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(node_id, sender) => self.add_sender(node_id, sender),
            DroneCommand::SetPacketDropRate(pdr) => self.set_pdr(pdr),
            DroneCommand::Crash => unreachable!(),
            DroneCommand::RemoveSender(node_id) => self.remove_sender(node_id),
        }
    }

    /// ======================================================================
    /// LOGS FOR DEBUG MODE only for testing while dev
    /// ======================================================================

    fn log(&self, message: &str) {
        if self.debug {
            println!("{}", message);
        }
    }

    fn log_received(&self, packet: &Packet) {
        let message = format!("{} : ({}:{}) {} -> {} | {:?}",
                              format!("[DRONE {}]", self.id).purple(),
                              packet.session_id,
                              packet.get_fragment_index(),
                              "Received".yellow(),
                              match &packet.pack_type {
                                  PacketType::Ack(_) => "Ack".green(),
                                  PacketType::Nack(_) => "Nack".red(),
                                  PacketType::FloodRequest(_) => "FloodRequest".yellow(),
                                  PacketType::FloodResponse(_) => "FloodResponse".yellow(),
                                  PacketType::MsgFragment(_) => "Message".cyan(),
                              }, packet,
        );
        self.log(&message);
    }

    fn log_nack(&self, nack_type: &NackType, session_id: u64, fragment_index: u64) {
        let message = format!("{} : ({}:{}) {} -> {}",
                              format!("[DRONE {}]", self.id).purple(),
                              session_id,
                              fragment_index,
                              "SentNack".red(),
                              format!("{:?}", nack_type).red(),
        );
        self.log(&message);
    }

    fn log_forwarded(&self, packet: &Packet) {
        let message = format!("{} : ({}:{}) {} -> {} | {:?}",
                              format!("[DRONE {}]", self.id).purple(),
                              packet.session_id,
                              packet.get_fragment_index(),
                              "Forwarded".green(),
                              match &packet.pack_type {
                                  PacketType::Ack(_) => "Ack".green(),
                                  PacketType::Nack(_a) => "Nack".red(),
                                  PacketType::FloodRequest(_) => "FloodRequest".yellow(),
                                  PacketType::FloodResponse(_) => "FloodResponse".yellow(),
                                  PacketType::MsgFragment(_) => "Message".cyan(),
                              }, packet,
        );
        self.log(&message);
    }

    /// ======================================================================
    /// HANDLING -> HANDLE PACKETS
    /// ======================================================================

    pub fn forward_packet(&mut self, mut packet: Packet, fragment_index: u64) {
        if self.id == packet.routing_header.hops[packet.routing_header.hop_index] {
            if let Some(next_hop) = packet.routing_header.hops.get(packet.routing_header.hop_index + 1) {
                packet.routing_header.hop_index += 1;
                if let Some(sender) = self.packet_send.get(next_hop) {
                    sender.send(packet.clone()).unwrap();                    // forwarding packet to next_hop
                    self.controller_send.send(PacketSent(packet.clone())).unwrap();  // sending confirmation to the SC
                    self.log_forwarded(&packet);
                } else {
                    self.send_nack(packet.clone(), fragment_index, NackType::ErrorInRouting(*next_hop));
                }
            } else {
                self.send_nack(packet, fragment_index, NackType::DestinationIsDrone);
            }
        } else {
            self.send_nack(packet, fragment_index, NackType::UnexpectedRecipient(self.id));
        }
    }

    pub fn handle_fragment(&mut self, routing_header : SourceRoutingHeader, session_id : u64, fragment: Fragment) {
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
            self.send_nack(packet.clone(), fragment.fragment_index, NackType::Dropped);
            self.controller_send.send(PacketDropped(packet.clone())).unwrap()       // sending confirmation of drop to SC
        }
    }

    pub fn should_drop_packet(&self) -> bool {
        random::<f32>() <= self.pdr
    }

    pub fn handle_ndp(&mut self, mut flood_request: FloodRequest, session_id: u64) {
        let prev_node = flood_request.path_trace.last().unwrap().0;
        flood_request.increment(self.id, NodeType::Drone);
        if self.received_flood_ids.contains(&(flood_request.flood_id, flood_request.initiator_id)) {
            self.forward_flood_response(&mut flood_request, session_id);
            return;
        } else {
            self.received_flood_ids.insert((flood_request.flood_id, flood_request.initiator_id));
            let neighbors: Vec<(NodeId, Sender<Packet>)> = self.packet_send
                .iter()
                .filter(|(&neighbor_id, _)| neighbor_id != prev_node)
                .map(|(&neighbor_id, sender)| (neighbor_id, sender.clone()))
                .collect();
            if !neighbors.is_empty() {
                for (_n_id, n_send) in neighbors {
                    let packet = Packet::new_flood_request(SourceRoutingHeader::empty_route(), session_id, flood_request.clone());
                    n_send.send(packet.clone()).unwrap();
                    self.log_forwarded(&packet);
                }
            } else {
                self.forward_flood_response(&mut flood_request, session_id);
                return;
            }
        }
    }

    pub fn forward_flood_response(&mut self, flood_request: &mut FloodRequest, session_id: u64) {
        let packet = flood_request.generate_response(session_id);
        self.forward_packet(packet, 0);
    }

    pub fn send_nack(&mut self, mut packet: Packet, fragment_index: u64, nack_type: NackType) {
        match packet.pack_type {
            PacketType::Nack(_) | PacketType::Ack(_) | PacketType::FloodResponse(_) => {
                self.controller_send.send(DroneEvent::ControllerShortcut(packet.clone())).unwrap();
            }
            _ => {
                packet.routing_header.hops[packet.routing_header.hop_index] = self.id;
                let nack = Nack { fragment_index, nack_type };
                let self_index = packet.routing_header
                    .hops
                    .iter()
                    .position(|&hop| hop == self.id as u8)
                    .unwrap_or(0);
                let reversed_hops: Vec<NodeId> = packet.routing_header.hops[..=self_index]
                    .iter()
                    .cloned()
                    .rev()
                    .collect();

                if let Some(sender) = self.packet_send.get(&reversed_hops[1]) {
                    sender.send(Packet {
                        pack_type: PacketType::Nack(nack.clone()),
                        routing_header: SourceRoutingHeader {
                            hop_index: 1,
                            hops: reversed_hops,
                        },
                        session_id: packet.session_id,
                    }).unwrap();
                    self.log_nack(&nack_type, packet.session_id, fragment_index);
                }
            }
        }
    }

    /// ======================================================================
    /// HANDLING -> HANDLE COMMANDS
    /// ======================================================================

    pub fn add_sender(&mut self, node_id: NodeId, sender: Sender<Packet>) {
        if let Some(_) = self.packet_send.get(&node_id) {
            println!("Error while trying to add sender id: {}, from drone #{}: Sender id already exists!", node_id, self.id);
        } else {
            self.packet_send.insert(node_id, sender);
            println!("Added sender id {} to drone #{}", node_id, self.id);
        }
    }

    pub fn set_pdr(&mut self, pdr: f32) {
        if pdr >= 0. && pdr <= 1. {
            self.pdr = pdr;
            println!("Set PDR: Updated for drone #{} to: {}", self.id, pdr);
        } else {
            println!("Set PDR: Invalid PDR for drone #{}", self.id);
        }
    }

    pub fn remove_sender(&mut self, node_id: NodeId) {
        if let Some(_) = self.packet_send.get(&node_id) {
            self.packet_send.remove(&node_id);
            println!("Removed sender id: {}, from drone #{}", node_id, self.id);
        } else {
            println!("Error while trying to remove sender id: {}, from drone #{}: Sender id does not exist!", node_id, self.id);
        }
    }

}