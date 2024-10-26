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
use crate::error::error::BellandeMeshError;
use crate::node::node::{Node, NodeId, PublicKey};
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

    pub fn load_data_chunks(&self, node_id: &NodeId) -> Result<Vec<DataChunk>, BellandeMeshError> {
        let chunks_dir = self.data_dir.join("data_chunks").join(node_id.to_hex());

        if !chunks_dir.exists() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();

        const SUPPORTED_EXTENSIONS: [&str; 2] = ["bellande", "dat"];

        for entry in std::fs::read_dir(chunks_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
                if SUPPORTED_EXTENSIONS.contains(&extension) {
                    match self.read_chunk_file(&path) {
                        Ok(chunk) => chunks.push(chunk),
                        Err(e) => {
                            eprintln!("Error reading chunk file {:?}: {}", path, e);
                            continue;
                        }
                    }
                }
            }
        }

        chunks.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(chunks)
    }

    fn read_chunk_file(&self, path: &Path) -> Result<DataChunk, BellandeMeshError> {
        let file = self.get_connection(path)?;
        let reader = BufReader::new(file);

        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| BellandeMeshError::Custom("Invalid file extension".to_string()))?;

        match extension {
            "bellande" => bincode::deserialize_from(reader).map_err(|e| {
                BellandeMeshError::Deserialization(format!(
                    "Failed to deserialize .bellande file: {}",
                    e
                ))
            }),
            "dat" => bincode::deserialize_from(reader).map_err(|e| {
                BellandeMeshError::Deserialization(format!(
                    "Failed to deserialize .dat file: {}",
                    e
                ))
            }),
            _ => Err(BellandeMeshError::Custom(
                "Unsupported file extension".to_string(),
            )),
        }
    }

    pub fn save_data_chunk(
        &self,
        node_id: &NodeId,
        chunk: &DataChunk,
        use_bellande: bool,
    ) -> Result<(), BellandeMeshError> {
        let chunks_dir = self.data_dir.join("data_chunks").join(node_id.to_hex());
        std::fs::create_dir_all(&chunks_dir)?;

        let extension = if use_bellande { "bellande" } else { "dat" };
        let file_name = format!("{}.{}", chunk.id.to_hex(), extension);
        let path = chunks_dir.join(file_name);

        let file = self.get_connection(&path)?;
        bincode::serialize_into(file, chunk)
            .map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;

        Ok(())
    }

    pub async fn migrate_chunk_format(
        &self,
        node_id: &NodeId,
        to_bellande: bool,
    ) -> Result<(), BellandeMeshError> {
        let chunks = self.load_data_chunks(node_id)?;

        let chunks_dir = self.data_dir.join("data_chunks").join(node_id.to_hex());
        let temp_dir = chunks_dir.with_extension("temp");
        std::fs::create_dir_all(&temp_dir)?;

        for chunk in &chunks {
            let extension = if to_bellande { "bellande" } else { "dat" };
            let file_name = format!("{}.{}", chunk.id.to_hex(), extension);
            let path = temp_dir.join(file_name);

            let file = self.get_connection(&path)?;
            bincode::serialize_into(file, chunk)
                .map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;
        }

        let backup_dir = chunks_dir.with_extension("backup");
        if chunks_dir.exists() {
            std::fs::rename(&chunks_dir, &backup_dir)?;
        }

        std::fs::rename(&temp_dir, &chunks_dir)?;

        if backup_dir.exists() {
            std::fs::remove_dir_all(backup_dir)?;
        }

        Ok(())
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
