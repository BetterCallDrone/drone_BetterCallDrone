#[allow(unused)]
use crossbeam_channel::{select_biased, Receiver, Sender};
use std::collections::{HashMap, HashSet};
use std::env;
use colored::Colorize;
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

    /// Additional parameters specific to `BetterCallDrone`.
    ///
    /// - `received_flood_ids`: A `HashSet` that contains pairs of `flood_id` and `initiator_id`
    ///   to track flood requests that have already been processed.
    /// - `debug`: A flag indicating whether debug mode is enabled.
    received_flood_ids: HashSet<(u64, NodeId)>,
    debug: bool,
}

impl Drone for BetterCallDrone {
    /// Creates a new `BetterCallDrone` instance.
    ///
    /// # Parameters
    /// - `id`: The ID of the drone.
    /// - `controller_send`: A channel sender for sending events to the simulation controller.
    /// - `controller_recv`: A channel receiver for receiving commands from the simulation controller.
    /// - `packet_recv`: A channel receiver for incoming packets.
    /// - `packet_send`: A `HashMap` of neighboring nodes (IDs) and their corresponding packet senders.
    /// - `pdr`: The Packet Drop Rate, represented as a float between 0 and 1.
    ///
    /// # Returns
    /// A newly initialized instance of `BetterCallDrone`.
    ///
    /// # Notes
    /// - **Debug Mode**: The `debug` field is automatically enabled if the environment variable `BCD_DEBUG` is set.
    /// - **Flood Tracking**: The `received_flood_ids` field is initialized as an empty `HashSet`.
    fn new(
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Self {
        let debug_check = env::var("BCD_DEBUG").is_ok();
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,

            received_flood_ids: HashSet::new(),
            debug: debug_check,
        }
    }

    /// Starts the main run loop for the drone. It listens for commands and packets, and processes them accordingly.
    fn run(&mut self) {
        self.log(&format!("{}","Successfully spawned and started".green()));
        loop {
            select_biased! {
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        if let DroneCommand::Crash = command {
                            self.log(&format!("{}","Received Crash Command from SC".cyan()));
                            self.crash_drone();
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
        self.log(&format!("{}","Successfully stopped".green()));
    }
}

impl BetterCallDrone {
    /// Handles incoming packets based on their type.
    ///
    /// # Parameters
    /// - `packet`: The incoming packet to process.
    pub fn handle_packet(&mut self, packet: Packet) {
        self.log_received(&packet);
        match packet.pack_type {
            PacketType::Nack(_) | PacketType::Ack(_) | PacketType::FloodResponse(_) => self.forward_packet(packet, 0),
            PacketType::MsgFragment(fragment) => self.handle_fragment(&packet.routing_header, packet.session_id, &fragment),
            PacketType::FloodRequest(flood_request) => self.handle_ndp(flood_request, packet.session_id),
        }
    }

    /// Processes commands sent by the simulation controller.
    ///
    /// # Parameters
    /// - `command`: The command to handle.
    pub fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(node_id, sender) => self.add_sender(node_id, sender),
            DroneCommand::SetPacketDropRate(pdr) => self.set_pdr(pdr),
            DroneCommand::Crash => unreachable!(),
            DroneCommand::RemoveSender(node_id) => self.remove_sender(node_id),
        }
    }

    /// The following functions handle the debug functionality -------------------------------------
    ///
    /// Prints to console if debug mode is enabled.
    ///
    /// # Parameters
    /// - `message`: The message to print.
    fn log(&self, message: &str) {
        if self.debug {
            let m = format!("{} {message}",
                            format!("[BCDRONE #{}]", self.id).purple(),);
            println!("{m}");
        }
    }

    /// Generates the message to print in case of received packet.
    ///
    /// # Parameters
    /// - `packet`: The packet received.
    fn log_received(&self, packet: &Packet) {
        let message = format!("({}:{}) | {} -> {} | {}",
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

    /// Generates the message to print in case of nack.
    ///
    /// # Parameters
    /// - `nack_type`: The type of nack.
    /// - `session_id`: session id of the packet.
    /// - `fragment_index`: fragment index of the packet.
    /// - `through_sc`: check if the nack is sent through SC or not.
    fn log_nack(&self, nack_type: NackType, session_id: u64, fragment_index: u64, through_sc: bool) {
        let mut sent = "SentNack";
        if through_sc {sent = "SentNack through SC";};
        let message = format!("({}:{}) | {} -> {}",
                              session_id,
                              fragment_index,
                              sent.red(),
                              format!("{nack_type:?}").red(),
        );
        self.log(&message);
    }

    /// Generates the message to print in case of forwarder packet
    ///
    /// # Parameters
    /// - `packet`: The packet received.
    fn log_forwarded(&self, packet: &Packet) {
        let message = format!("({}:{}) | {} -> {} | {}",
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


    /// The following functions handle the processing of packets -----------------------------------
    ///
    /// Forwards a packet to the next node in its routing path.
    ///
    /// # Parameters
    /// - `packet`: The packet to forward.
    /// - `fragment_index`: The index of the fragment being forwarded.
    pub fn forward_packet(&mut self, mut packet: Packet, fragment_index: u64) {
        if self.id == packet.routing_header.hops[packet.routing_header.hop_index] {
            if let Some(next_hop) = packet.routing_header.hops.get(packet.routing_header.hop_index + 1) {
                packet.routing_header.hop_index += 1;
                if let Some(sender) = self.packet_send.get(next_hop) {
                    if let Err(e) = sender.send(packet.clone()) {
                        self.log(&format!("{} {}","Error in Forwarding Packet", e));
                    } else {
                        self.log_forwarded(&packet);
                    }

                    if let Err(e) = self.controller_send.send(PacketSent(packet.clone())) {
                        self.log(&format!("{} {}","Error in Sending `PacketSent` to SC: ", e));
                    } else {
                        self.log(&format!("{}","Event PacketSent sent to SC".green()));
                    }
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

    /// Handles a fragment packet, deciding whether to forward or drop it.
    ///
    /// # Parameters
    /// - `routing_header`: The routing header of the packet.
    /// - `session_id`: The session ID of the packet.
    /// - `fragment`: The fragment to handle.
    pub fn handle_fragment(&mut self, routing_header : &SourceRoutingHeader, session_id : u64, fragment: &Fragment) {
        let rh = routing_header.clone();
        let packet = Packet {
            routing_header: rh,
            session_id,
            pack_type: PacketType::MsgFragment(fragment.clone()),
        };
        if self.should_drop_packet() {
            self.send_nack(packet.clone(), fragment.fragment_index, NackType::Dropped);
            if let Err(e) = self.controller_send.send(PacketDropped(packet.clone())) {
                self.log(&format!("{} {}","Error in Sending `PacketDropped` to SC: ", e));
            } else {
                self.log(&format!("{}","Event PacketDropped sent to SC".green()));
            }
        } else {
            let index = fragment.fragment_index;
            self.forward_packet(packet, index);
        }
    }

    /// Determines whether a packet should be dropped based on the PDR.
    ///
    /// # Returns
    /// `true` if the packet should be dropped, otherwise `false`.
    #[must_use]
    pub fn should_drop_packet(&self) -> bool {
        random::<f32>() <= self.pdr
    }

    /// Handles a flood request packet (Network Discovery Protocol).
    ///
    /// # Parameters
    /// - `flood_request`: The flood request to process.
    /// - `session_id`: The session ID of the request.
    pub fn handle_ndp(&mut self, mut flood_request: FloodRequest, session_id: u64) {
        let prev_node = flood_request.path_trace.last().unwrap().0;
        flood_request.increment(self.id, NodeType::Drone);
        if self.received_flood_ids.contains(&(flood_request.flood_id, flood_request.initiator_id)) {
            self.forward_flood_response(&mut flood_request, session_id);
        } else {
            self.received_flood_ids.insert((flood_request.flood_id, flood_request.initiator_id));
            let neighbors: Vec<(NodeId, Sender<Packet>)> = self.packet_send
                .iter()
                .filter(|(&neighbor_id, _)| neighbor_id != prev_node)
                .map(|(&neighbor_id, sender)| (neighbor_id, sender.clone()))
                .collect();
            if neighbors.is_empty() {
                self.forward_flood_response(&mut flood_request, session_id);
            } else {
                for (_n_id, n_send) in neighbors {
                    let packet = Packet::new_flood_request(SourceRoutingHeader::empty_route(), session_id, flood_request.clone());
                    if let Err(e) = n_send.send(packet.clone()) {
                        self.log(&format!("{} {}","Error in Sending FloodRequest: ", e));
                    } else {
                        self.log_forwarded(&packet);
                    }
                }
            }
        }
    }

    /// Generates and Sends a Flood Response.
    ///
    /// # Parameters
    /// - `flood_request`: The original `FloodRequest`.
    /// - `session_id`: The session ID of the request.
    pub fn forward_flood_response(&mut self, flood_request: &mut FloodRequest, session_id: u64) {
        let packet = flood_request.generate_response(session_id);
        self.forward_packet(packet, 0);
    }

    /// Generates and Sends a NACK through the reversed path or through SC.
    ///
    /// # Parameters
    /// - `packet`: The original packet causing the NACK.
    /// - `fragment_index`: The index of the fragment.
    /// - `nack_type`: The type of NACK being sent.
    pub fn send_nack(&mut self, mut packet: Packet, fragment_index: u64, nack_type: NackType) {
        match packet.pack_type {
            PacketType::Nack(_) | PacketType::Ack(_) | PacketType::FloodResponse(_) => {
                if let Err(e) = self.controller_send.send(DroneEvent::ControllerShortcut(packet.clone())) {
                    self.log(&format!("{} {}","Error in Sending Nack through SC: ", e));
                } else {
                    self.log_nack(nack_type, packet.session_id, fragment_index, true);
                }
            }
            _ => {
                packet.routing_header.hops[packet.routing_header.hop_index] = self.id;
                let nack = Nack { fragment_index, nack_type };
                let self_index = packet.routing_header
                    .hops
                    .iter()
                    .position(|&hop| hop == self.id)
                    .unwrap_or(0);
                let reversed_hops: Vec<NodeId> = packet.routing_header.hops[..=self_index]
                    .iter()
                    .copied()
                    .rev()
                    .collect();

                if let Some(sender) = self.packet_send.get(&reversed_hops[1]) {
                    if let Err(e) = sender.send(Packet {
                        pack_type: PacketType::Nack(nack.clone()),
                        routing_header: SourceRoutingHeader {
                            hop_index: 1,
                            hops: reversed_hops,
                        },
                        session_id: packet.session_id,
                    }) {
                        self.log(&format!("{} {}","Error in Sending Nack: ", e));
                    } else {
                        self.log_nack(nack_type, packet.session_id, fragment_index, false);
                    }
                }
            }
        }
    }

    /// The following functions handle the commands sent to the drone by the SC --------------------
    ///
    /// Adds a sender to the list of neighbors.
    ///
    /// # Parameters
    /// - `node_id`: The ID of the node to add.
    /// - `sender`: The sender channel associated with the node.
    pub fn add_sender(&mut self, node_id: NodeId, sender: Sender<Packet>) {
        self.log(&format!("{}","Received AddSender Command from SC".cyan()));
        if self.packet_send.contains_key(&node_id) {
            self.log(&format!("{} -> {} {}","AddSender".cyan(),"Error while trying to remove sender id:".red(), node_id));
        } else {
            self.packet_send.insert(node_id, sender);
            self.log(&format!("{} -> {} {}","AddSender".cyan(),"Successfully added sender id:".green(), node_id));
        }
    }

    /// Sets the packet drop rate for the drone.
    ///
    /// # Parameters
    /// - `pdr`: The new packet drop rate.
    pub fn set_pdr(&mut self, pdr: f32) {
        self.log(&format!("{}","Received SetPacketDropRate Command from SC".cyan()));
        if (0.0..=1.0).contains(&pdr) {
            self.pdr = pdr;
            self.log(&format!("{} -> {} {}","SetPacketDropRate".cyan(),"Updated PDR to".green(), pdr));
        } else {
            self.log(&format!("{} -> {}{}{}","SetPacketDropRate".cyan(),"Invalid PDR (".red(), pdr, ")".red()));
        }
    }

    /// Removes a sender from the list of neighbors.
    ///
    /// # Parameters
    /// - `node_id`: The ID of the node to remove.
    pub fn remove_sender(&mut self, node_id: NodeId) {
        self.log(&format!("{}","Received RemoveSender Command from SC".cyan()));
        if self.packet_send.contains_key(&node_id) {
            self.packet_send.remove(&node_id);
            self.log(&format!("{} -> {} {}","RemoveSender".cyan(),"Successfully removed sender id:".green(), node_id));
        } else {
            self.log(&format!("{} -> {} {}","RemoveSender".cyan(),"Error while trying to remove sender id:".red(), node_id));
        }
    }

    /// Crashes the drone, clearing queued packets and stopping operation.
    pub fn crash_drone(&mut self){
        while let Ok(packet) = self.packet_recv.try_recv() {
            match &packet.pack_type {
                PacketType::MsgFragment(frag) => {
                    self.send_nack(packet.clone(), frag.fragment_index, NackType::ErrorInRouting(self.id));
                }
                PacketType::FloodRequest(_) => {}
                _ => self.forward_packet(packet, 0),
            }
        }
        self.log(&format!("{}, {}","Finished handling packets".green(),"Drone Crashed successfully".red()));
    }
}