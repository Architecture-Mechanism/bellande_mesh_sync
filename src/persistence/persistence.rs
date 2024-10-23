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
use crate::node::node::{DataChunk, Node, NodeId, PublicKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct SerializableNode {
    id: NodeId,
    address: std::net::SocketAddr,
    public_key: PublicKey,
    last_seen: SystemTime,
}

impl From<&Node> for SerializableNode {
    fn from(node: &Node) -> Self {
        SerializableNode {
            id: node.id,
            address: node.address,
            public_key: node.public_key.clone(),
            last_seen: node.last_seen,
        }
    }
}

impl SerializableNode {
    fn into_node(self) -> Node {
        Node {
            id: self.id,
            address: self.address,
            public_key: self.public_key,
            last_seen: self.last_seen,
            rtt: Default::default(),
            failed_queries: 0,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

pub struct PersistenceManager {
    data_dir: PathBuf,
    connections: Arc<Mutex<Vec<FileConnection>>>,
}

struct FileConnection {
    file: File,
}

impl PersistenceManager {
    pub fn new(data_dir: &str) -> Result<Self, BellandeMeshError> {
        let path = Path::new(data_dir);
        std::fs::create_dir_all(path)?;

        Ok(Self {
            data_dir: path.to_path_buf(),
            connections: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn save_node(&self, node: &Node) -> Result<(), BellandeMeshError> {
        let path = self
            .data_dir
            .join("nodes")
            .join(format!("{}.dat", node.id.to_hex()));

        std::fs::create_dir_all(path.parent().unwrap())?;
        let file = self.get_connection(&path)?;
        let writer = BufWriter::new(file);

        let serializable_node = SerializableNode::from(node);
        bincode::serialize_into(writer, &serializable_node)
            .map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;

        Ok(())
    }

    pub fn load_nodes(&self) -> Result<Vec<Node>, BellandeMeshError> {
        let nodes_dir = self.data_dir.join("nodes");
        if !nodes_dir.exists() {
            return Ok(Vec::new());
        }

        let mut nodes = Vec::new();
        for entry in std::fs::read_dir(nodes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("dat") {
                let file = self.get_connection(&path)?;
                let reader = BufReader::new(file);

                let serializable_node: SerializableNode = bincode::deserialize_from(reader)
                    .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))?;

                nodes.push(serializable_node.into_node());
            }
        }

        Ok(nodes)
    }

    pub fn save_data_chunk(
        &self,
        node_id: &NodeId,
        chunk: &DataChunk,
    ) -> Result<(), BellandeMeshError> {
        let path = self
            .data_dir
            .join("data_chunks")
            .join(node_id.to_hex())
            .join(format!("{}.dat", chunk.id.to_hex()));

        std::fs::create_dir_all(path.parent().unwrap())?;
        let file = self.get_connection(&path)?;
        let writer = BufWriter::new(file);

        bincode::serialize_into(writer, chunk)
            .map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;

        Ok(())
    }

    pub fn load_data_chunks(
        &self,
        node_id: &NodeId,
    ) -> Result<HashMap<NodeId, DataChunk>, BellandeMeshError> {
        let chunks_dir = self.data_dir.join("data_chunks").join(node_id.to_hex());

        if !chunks_dir.exists() {
            return Ok(HashMap::new());
        }

        let mut chunks = HashMap::new();
        for entry in std::fs::read_dir(chunks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("dat") {
                let file = self.get_connection(&path)?;
                let reader = BufReader::new(file);

                let chunk: DataChunk = bincode::deserialize_from(reader)
                    .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))?;
                chunks.insert(chunk.id, chunk);
            }
        }

        Ok(chunks)
    }

    fn get_connection(&self, path: &Path) -> Result<File, BellandeMeshError> {
        let mut connections = self
            .connections
            .lock()
            .map_err(|_| BellandeMeshError::LockError)?;

        if let Some(conn) = connections
            .iter_mut()
            .find(|c| c.file.metadata().unwrap().ino() == path.metadata().unwrap().ino())
        {
            Ok(conn.file.try_clone()?)
        } else {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(path)?;
            connections.push(FileConnection {
                file: file.try_clone()?,
            });
            Ok(file)
        }
    }
}
