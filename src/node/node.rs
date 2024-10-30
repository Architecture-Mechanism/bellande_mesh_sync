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

use crate::data::data::DataChunk;
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn new() -> Self {
        let mut id = [0u8; 32];
        thread_rng().fill_bytes(&mut id);
        NodeId(id)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut id = [0u8; 32];
        let len = std::cmp::min(bytes.len(), 32);
        id[..len].copy_from_slice(&bytes[..len]);
        NodeId(id)
    }

    pub fn distance(&self, other: &NodeId) -> NodeId {
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = self.0[i] ^ other.0[i];
        }
        NodeId(result)
    }

    pub fn as_ref(&self) -> &[u8] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        PublicKey(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Ping {
        sender: NodeId,
        token: u64,
    },
    Pong {
        sender: NodeId,
        token: u64,
    },
    Store {
        sender: NodeId,
        key: Vec<u8>,
        value: Vec<u8>,
        token: u64,
    },
    FindNode {
        sender: NodeId,
        target: NodeId,
        token: u64,
    },
    FindValue {
        sender: NodeId,
        key: Vec<u8>,
        token: u64,
    },
    Nodes {
        sender: NodeId,
        nodes: Vec<Node>,
        token: u64,
    },
    Value {
        sender: NodeId,
        key: Vec<u8>,
        value: Vec<u8>,
        token: u64,
    },
    JoinRequest {
        id: NodeId,
        public_key: PublicKey,
    },
    JoinResponse {
        accepted: bool,
        nodes: Vec<Node>,
    },
    DataSync {
        chunks: Vec<DataChunk>,
    },
    DataRequest {
        ids: Vec<NodeId>,
    },
    Heartbeat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub address: SocketAddr,
    pub public_key: PublicKey,
    pub last_seen: SystemTime,
    pub rtt: Duration,
    pub failed_queries: u32,
    #[serde(skip)]
    pub data: Arc<RwLock<HashMap<NodeId, DataChunk>>>,
}

impl Node {
    pub fn new(id: NodeId, address: SocketAddr, public_key: PublicKey) -> Self {
        Self {
            id,
            address,
            public_key,
            last_seen: SystemTime::now(),
            rtt: Duration::from_secs(0),
            failed_queries: 0,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
    }

    pub fn mark_failed(&mut self) {
        self.failed_queries += 1;
    }

    pub fn update_rtt(&mut self, rtt: Duration) {
        self.rtt = rtt;
        self.last_seen = SystemTime::now();
        self.failed_queries = 0;
    }

    pub fn add_data_chunk(&self, chunk: DataChunk) -> bool {
        let data = self.data.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let mut guard = data.write().await;
            guard.insert(chunk.id, chunk);
            true
        })
    }

    pub fn get_data_chunk(&self, id: &NodeId) -> Option<DataChunk> {
        let data = self.data.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let guard = data.read().await;
            guard.get(id).cloned()
        })
    }

    pub fn remove_old_data(&self, max_age: Duration) -> bool {
        let data = self.data.clone();
        tokio::runtime::Handle::current().block_on(async move {
            let mut guard = data.write().await;
            let now = SystemTime::now();
            guard.retain(|_, chunk| {
                now.duration_since(chunk.last_modified)
                    .map(|age| age <= max_age)
                    .unwrap_or(false)
            });
            true
        })
    }

    pub fn is_alive(&self, timeout: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.last_seen)
            .map(|duration| duration < timeout)
            .unwrap_or(false)
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Node {}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}
