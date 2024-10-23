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

use crate::config::config::Config;
use crate::error::error::BellandeMeshError;
use crate::node::node::{DataChunk, Message, Node, NodeId, PublicKey};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

pub struct BellandeMeshSync {
    config: Arc<Config>,
    nodes: Arc<RwLock<Vec<Node>>>,
    running: Arc<RwLock<bool>>,
}

impl BellandeMeshSync {
    pub fn new(config: &Config) -> Result<Self, BellandeMeshError> {
        Ok(Self {
            config: Arc::new(config.clone()),
            nodes: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(true)),
        })
    }

    pub fn start(&self) -> Result<(), BellandeMeshError> {
        let mut running = self
            .running
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        *running = true;
        drop(running);

        self.start_listener()?;
        self.start_maintenance_tasks();
        Ok(())
    }

    pub fn stop(&self) -> Result<(), BellandeMeshError> {
        let mut running = self
            .running
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        *running = false;
        Ok(())
    }

    fn is_running(&self) -> Result<bool, BellandeMeshError> {
        self.running
            .read()
            .map_err(|_| BellandeMeshError::LockError)
            .map(|guard| *guard)
    }

    fn start_listener(&self) -> Result<(), BellandeMeshError> {
        let listener = TcpListener::bind(&self.config.listen_address)?;
        let mesh = self.clone();

        thread::spawn(move || {
            for stream in listener.incoming() {
                if !mesh.is_running().unwrap_or(false) {
                    break;
                }

                match stream {
                    Ok(stream) => {
                        let mesh_clone = mesh.clone();
                        thread::spawn(move || {
                            if let Err(e) = mesh_clone.handle_connection(stream) {
                                eprintln!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => eprintln!("Accept error: {}", e),
                }
            }
        });

        Ok(())
    }

    fn start_maintenance_tasks(&self) {
        // Start sync task
        let mesh = self.clone();
        thread::spawn(move || {
            while let Ok(true) = mesh.is_running() {
                if let Err(e) = mesh.sync_with_peers() {
                    eprintln!("Sync error: {}", e);
                }
                thread::sleep(Duration::from_secs(60));
            }
        });

        // Start cleanup task
        let mesh = self.clone();
        thread::spawn(move || {
            while let Ok(true) = mesh.is_running() {
                if let Err(e) = mesh.cleanup_dead_nodes() {
                    eprintln!("Cleanup error: {}", e);
                }
                thread::sleep(Duration::from_secs(300));
            }
        });
    }

    fn handle_connection(&self, mut stream: TcpStream) -> Result<(), BellandeMeshError> {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;

        let running = self
            .running
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;
        while *running {
            let message = self.read_message(&mut stream)?;
            self.handle_message(message, &mut stream)?;
        }

        Ok(())
    }

    fn read_message(&self, stream: &mut TcpStream) -> Result<Message, BellandeMeshError> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        stream.read_exact(&mut msg_buf)?;

        bincode::deserialize(&msg_buf)
            .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))
    }

    fn write_message(
        &self,
        stream: &mut TcpStream,
        message: &Message,
    ) -> Result<(), BellandeMeshError> {
        let data = bincode::serialize(message)
            .map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;
        let len = (data.len() as u32).to_be_bytes();

        stream.write_all(&len)?;
        stream.write_all(&data)?;
        stream.flush()?;

        Ok(())
    }

    fn handle_message(
        &self,
        message: Message,
        stream: &mut TcpStream,
    ) -> Result<(), BellandeMeshError> {
        match message {
            Message::JoinRequest { id, public_key } => {
                let peer_addr = stream.peer_addr()?;
                self.handle_join_request(id, public_key, peer_addr)?;

                let nodes = self
                    .nodes
                    .read()
                    .map_err(|_| BellandeMeshError::LockError)?;
                let response = Message::JoinResponse {
                    accepted: true,
                    nodes: nodes.clone(),
                };
                self.write_message(stream, &response)?;
            }
            Message::DataSync { chunks } => {
                self.handle_data_sync(chunks)?;
            }
            Message::DataRequest { ids } => {
                self.handle_data_request(&ids, stream)?;
            }
            Message::Heartbeat => {
                let addr = stream.peer_addr()?;
                self.update_node_last_seen(addr)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_join_request(
        &self,
        id: NodeId,
        public_key: PublicKey,
        addr: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let new_node = Node::new(id, addr, public_key);
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;

        if !nodes.iter().any(|n| n.id == new_node.id) {
            nodes.push(new_node);
        }

        Ok(())
    }

    fn handle_data_sync(&self, chunks: Vec<DataChunk>) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;

        for chunk in chunks {
            if let Some(node) = nodes.iter().find(|n| n.id == chunk.author) {
                let _ = node.add_data_chunk(chunk);
            }
        }

        Ok(())
    }

    fn handle_data_request(
        &self,
        ids: &[NodeId],
        stream: &mut TcpStream,
    ) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;
        let mut chunks = Vec::new();

        for node in nodes.iter() {
            for id in ids {
                if let Some(chunk) = node.get_data_chunk(id) {
                    chunks.push(chunk);
                }
            }
        }

        let response = Message::DataSync { chunks };
        self.write_message(stream, &response)?;

        Ok(())
    }

    fn update_node_last_seen(&self, addr: SocketAddr) -> Result<(), BellandeMeshError> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;

        if let Some(node) = nodes.iter_mut().find(|n| n.address == addr) {
            node.update_last_seen();
        }

        Ok(())
    }

    fn sync_with_peers(&self) -> Result<(), BellandeMeshError> {
        // Take a snapshot of current nodes to avoid holding the lock
        let nodes = {
            let nodes_guard = self
                .nodes
                .read()
                .map_err(|_| BellandeMeshError::LockError)?;
            nodes_guard.clone()
        };

        for node in nodes {
            if let Ok(mut stream) = TcpStream::connect(node.address) {
                let chunks = {
                    let data = node.data.read().map_err(|_| BellandeMeshError::LockError)?;
                    data.keys().cloned().collect::<Vec<_>>()
                };

                let request = Message::DataRequest { ids: chunks };
                if let Err(e) = self.write_message(&mut stream, &request) {
                    eprintln!("Failed to sync with {}: {}", node.address, e);
                    continue;
                }

                // Handle response
                if let Ok(Message::DataSync { chunks }) = self.read_message(&mut stream) {
                    self.handle_data_sync(chunks)?;
                }
            }
        }

        Ok(())
    }

    fn cleanup_dead_nodes(&self) -> Result<(), BellandeMeshError> {
        let timeout = Duration::from_secs(self.config.node_timeout);
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;

        nodes.retain(|node| {
            let is_alive = node.is_alive(timeout);
            if !is_alive {
                eprintln!("Removing dead node: {}", node.address);
            }
            is_alive
        });

        Ok(())
    }
}

impl Clone for BellandeMeshSync {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            nodes: Arc::clone(&self.nodes),
            running: Arc::clone(&self.running),
        }
    }
}
