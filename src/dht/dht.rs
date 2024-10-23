// Copyright (C) 2024 Bellande Architecture Mechanism Research Innovation Center, Ronaldson Bellande

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::error::error::BellandeMeshError;
use crate::node::node::{Message, Node, NodeId, PublicKey};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

const ALPHA: usize = 3;
const K: usize = 20;
const BUCKET_REFRESH_INTERVAL: Duration = Duration::from_secs(3600);
const REPLICATION_INTERVAL: Duration = Duration::from_secs(3600);
const NODE_TIMEOUT: Duration = Duration::from_secs(900);
const PING_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
struct KBucket {
    nodes: Vec<Node>,
    last_updated: Instant,
}

impl KBucket {
    fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(K),
            last_updated: Instant::now(),
        }
    }

    fn add_node(&mut self, node: Node) {
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.id == node.id) {
            existing.last_seen = node.last_seen;
            existing.rtt = node.rtt;
            return;
        }

        if self.nodes.len() < K {
            self.nodes.push(node);
        } else if let Some(pos) = self.nodes.iter().position(|n| n.failed_queries > 0) {
            self.nodes[pos] = node;
        }

        self.last_updated = Instant::now();
        self.nodes.sort_by_key(|n| n.rtt);
    }

    fn remove_node(&mut self, id: &NodeId) {
        self.nodes.retain(|n| n.id != *id);
    }

    fn get_nodes(&self) -> Vec<Node> {
        self.nodes.clone()
    }
}

#[derive(Clone, Debug)]
pub struct StorageValue {
    pub data: Vec<u8>,
    pub timestamp: SystemTime,
    pub ttl: Duration,
}

impl StorageValue {
    pub fn new(data: Vec<u8>, ttl: Duration) -> Self {
        Self {
            data,
            timestamp: SystemTime::now(),
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now()
            .duration_since(self.timestamp)
            .map(|elapsed| elapsed >= self.ttl)
            .unwrap_or(true)
    }
}

struct RoutingTable {
    buckets: Vec<KBucket>,
    local_id: NodeId,
}

impl RoutingTable {
    fn new(local_id: NodeId) -> Self {
        Self {
            buckets: (0..256).map(|_| KBucket::new()).collect(),
            local_id,
        }
    }

    fn add_node(&mut self, node: Node) {
        let bucket_idx = self.bucket_index(&node.id);
        self.buckets[bucket_idx].add_node(node);
    }

    fn get_closest_nodes(&self, target: &NodeId) -> Vec<Node> {
        let mut nodes = Vec::new();
        for bucket in &self.buckets {
            nodes.extend(bucket.get_nodes());
        }

        nodes.sort_by_cached_key(|node| node.id.distance(target));
        nodes.truncate(K);
        nodes
    }

    fn bucket_index(&self, id: &NodeId) -> usize {
        let distance = self.local_id.distance(id);
        let distance_bytes = distance.as_ref();
        let mut leading_zeros = 0;

        for &byte in distance_bytes {
            if byte == 0 {
                leading_zeros += 8;
            } else {
                leading_zeros += byte.leading_zeros() as usize;
                break;
            }
        }

        (255 - leading_zeros).min(255)
    }
}

pub struct DhtWrapper {
    routing_table: Arc<RwLock<RoutingTable>>,
    storage: Arc<RwLock<HashMap<Vec<u8>, StorageValue>>>,
    socket: Arc<UdpSocket>,
    pending_requests: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl DhtWrapper {
    pub fn new(bind_addr: SocketAddr, local_key: &[u8]) -> Result<Self, BellandeMeshError> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;

        let local_id = NodeId::from_bytes(local_key);
        let routing_table = RoutingTable::new(local_id);

        let wrapper = Self {
            routing_table: Arc::new(RwLock::new(routing_table)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            socket: Arc::new(socket),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        };

        wrapper.start_background_tasks();
        Ok(wrapper)
    }

    fn start_background_tasks(&self) {
        let dht = self.clone();
        thread::spawn(move || loop {
            thread::sleep(PING_INTERVAL);
            if let Err(e) = dht.maintenance_task() {
                eprintln!("Maintenance error: {}", e);
            }
        });

        let dht = self.clone();
        thread::spawn(move || loop {
            if let Err(e) = dht.handle_incoming_messages() {
                eprintln!("Message handling error: {}", e);
                thread::sleep(Duration::from_millis(100));
            }
        });
    }

    fn maintenance_task(&self) -> Result<(), BellandeMeshError> {
        // Clean up expired requests
        {
            let mut requests = self
                .pending_requests
                .lock()
                .map_err(|_| BellandeMeshError::LockError)?;
            requests.retain(|_, time| time.elapsed() < Duration::from_secs(60));
        }

        // Get outdated buckets
        let outdated_buckets = {
            let table = self
                .routing_table
                .read()
                .map_err(|_| BellandeMeshError::LockError)?;
            table
                .buckets
                .iter()
                .enumerate()
                .filter(|(_, bucket)| bucket.last_updated.elapsed() > BUCKET_REFRESH_INTERVAL)
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };

        // Refresh outdated buckets
        for i in outdated_buckets {
            let target_id = NodeId::new();
            self.find_node(&target_id)?;
        }

        Ok(())
    }

    fn handle_incoming_messages(&self) -> Result<(), BellandeMeshError> {
        let mut buf = vec![0u8; 65536];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    let message = bincode::deserialize(&buf[..len])
                        .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))?;
                    self.handle_message(message, src)?;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => return Err(BellandeMeshError::IoError(e)),
            }
        }
    }

    fn handle_message(&self, message: Message, src: SocketAddr) -> Result<(), BellandeMeshError> {
        match message {
            Message::Ping { sender, token } => {
                let response = Message::Pong {
                    sender: self.get_local_id()?,
                    token,
                };
                self.send_message(&src, &response)?;
            }
            Message::Pong { sender, token } => {
                if let Some(start_time) = self
                    .pending_requests
                    .lock()
                    .map_err(|_| BellandeMeshError::LockError)?
                    .remove(&token)
                {
                    let rtt = start_time.elapsed();
                    let mut node = Node::new(sender, src, PublicKey::new([0; 32]));
                    node.update_rtt(rtt);
                    self.routing_table
                        .write()
                        .map_err(|_| BellandeMeshError::LockError)?
                        .add_node(node);
                }
            }
            Message::FindNode {
                sender,
                target,
                token,
            } => {
                let nodes = self.get_closest_nodes(&target)?;
                let response = Message::Nodes {
                    sender: self.get_local_id()?,
                    nodes,
                    token,
                };
                self.send_message(&src, &response)?;
            }
            Message::Store {
                sender,
                key,
                value,
                token,
            } => {
                self.store(&key, &value, REPLICATION_INTERVAL)?;
                let response = Message::Pong {
                    sender: self.get_local_id()?,
                    token,
                };
                self.send_message(&src, &response)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn send_message(&self, addr: &SocketAddr, msg: &Message) -> Result<(), BellandeMeshError> {
        let data =
            bincode::serialize(msg).map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;
        self.socket.send_to(&data, addr)?;
        Ok(())
    }

    fn get_local_id(&self) -> Result<NodeId, BellandeMeshError> {
        Ok(self
            .routing_table
            .read()
            .map_err(|_| BellandeMeshError::LockError)?
            .local_id
            .clone())
    }

    fn get_closest_nodes(&self, target: &NodeId) -> Result<Vec<Node>, BellandeMeshError> {
        Ok(self
            .routing_table
            .read()
            .map_err(|_| BellandeMeshError::LockError)?
            .get_closest_nodes(target))
    }

    pub fn store(&self, key: &[u8], value: &[u8], ttl: Duration) -> Result<(), BellandeMeshError> {
        let storage_value = StorageValue::new(value.to_vec(), ttl);
        let mut storage = self
            .storage
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        storage.insert(key.to_vec(), storage_value);

        let target = NodeId::from_bytes(key);
        let closest_nodes = self.get_closest_nodes(&target)?;

        for node in closest_nodes {
            let token = rand::thread_rng().gen();
            let msg = Message::Store {
                sender: self.get_local_id()?,
                key: key.to_vec(),
                value: value.to_vec(),
                token,
            };
            let _ = self.send_message(&node.address, &msg);
        }

        Ok(())
    }

    fn cleanup_storage(&self) -> Result<(), BellandeMeshError> {
        let mut storage = self
            .storage
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        storage.retain(|_, value| !value.is_expired());
        Ok(())
    }

    pub fn find_node(&self, target: &NodeId) -> Result<Vec<Node>, BellandeMeshError> {
        let closest = self.get_closest_nodes(target)?;
        let mut queried_nodes = HashSet::new();
        let mut results = Vec::new();

        for node in closest {
            if queried_nodes.contains(&node.id) {
                continue;
            }

            let token = rand::thread_rng().gen();
            let msg = Message::FindNode {
                sender: self.get_local_id()?,
                target: target.clone(),
                token,
            };

            if let Ok(()) = self.send_message(&node.address, &msg) {
                queried_nodes.insert(node.id);
                results.push(node);
            }
        }

        results.sort_by_cached_key(|node| node.id.distance(target));
        results.truncate(K);
        Ok(results)
    }
}

impl Clone for DhtWrapper {
    fn clone(&self) -> Self {
        Self {
            routing_table: Arc::clone(&self.routing_table),
            storage: Arc::clone(&self.storage),
            socket: Arc::clone(&self.socket),
            pending_requests: Arc::clone(&self.pending_requests),
        }
    }
}
