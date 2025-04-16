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

use std::path::PathBuf;
use std::sync::Arc;

pub mod algorithm;
pub mod config;
pub mod data;
pub mod dht;
pub mod encryption;
pub mod error;
pub mod mesh;
pub mod metrics;
pub mod node;
pub mod persistence;
pub mod utilities;

pub use crate::config::config::Config;
pub use crate::encryption::encryption::PublicKey;
pub use crate::error::error::BellandeMeshError;
pub use crate::mesh::mesh::{BellandeMeshSync, NetworkStats};
pub use crate::metrics::metrics::MetricsManager;
pub use crate::node::node::{Node, NodeId};
pub use crate::persistence::persistence::PersistenceManager;

/// Configuration options for initializing the BellandeMeshSync system
#[derive(Debug, Clone)]
pub struct MeshOptions {
    /// Path to the TLS certificate file
    pub cert_path: Option<PathBuf>,
    /// Path to the TLS private key file
    pub key_path: Option<PathBuf>,
    /// Enable development mode (generates self-signed certificates)
    pub dev_mode: bool,
    /// Custom metrics collection interval in seconds
    pub metrics_interval: Option<u64>,
    /// Maximum number of concurrent connections
    pub max_connections: Option<usize>,
    /// Enable persistence layer
    pub enable_persistence: bool,
    /// Custom persistence directory
    pub persistence_path: Option<PathBuf>,
    /// Node timeout duration in seconds
    pub node_timeout: Option<u64>,
    /// Enable automatic node discovery
    pub enable_discovery: bool,
    /// Bootstrap nodes for initial connection
    pub bootstrap_nodes: Vec<String>,
    /// Enable encryption for all communications
    pub enable_encryption: bool,
    /// Custom TCP port for mesh communication
    pub tcp_port: Option<u16>,
    /// Custom UDP port for mesh communication
    pub udp_port: Option<u16>,
    /// Maximum retry attempts for failed connections
    pub max_retries: Option<u32>,
    /// Enable automatic data replication
    pub enable_replication: bool,
    /// Number of data replicas to maintain
    pub replication_factor: Option<u32>,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
            dev_mode: false,
            metrics_interval: Some(60),
            max_connections: Some(1000),
            enable_persistence: false,
            persistence_path: None,
            node_timeout: Some(300),
            enable_discovery: true,
            bootstrap_nodes: Vec::new(),
            enable_encryption: true,
            tcp_port: None,
            udp_port: None,
            max_retries: Some(3),
            enable_replication: true,
            replication_factor: Some(3),
        }
    }
}

/// Initialize the BellandeMeshSync System with default options
pub async fn init(config: Config) -> Result<BellandeMeshSync, BellandeMeshError> {
    init_with_options(config, MeshOptions::default()).await
}

/// Initialize the BellandeMeshSync System with custom options
pub async fn init_with_options(
    config: Config,
    options: MeshOptions,
) -> Result<BellandeMeshSync, BellandeMeshError> {
    // Initialize metrics manager
    let metrics = Arc::new(MetricsManager::new());

    // Initialize persistence if enabled
    let persistence_manager = if options.enable_persistence {
        let path = options
            .persistence_path
            .unwrap_or_else(|| PathBuf::from("data"));
        Some(PersistenceManager::new(path.to_str().unwrap_or("data"))?)
    } else {
        None
    };

    // Initialize the mesh network with appropriate certificates
    let bellande_mesh = match (options.cert_path, options.key_path, options.dev_mode) {
        (Some(cert), Some(key), false) => {
            BellandeMeshSync::new_with_certs(&config, cert, key, metrics.clone())?
        }
        (None, None, true) => {
            // Use default certificates for development
            let cert = PathBuf::from("certs/server.crt");
            let key = PathBuf::from("certs/server.key");
            BellandeMeshSync::new_with_certs(&config, cert, key, metrics.clone())?
        }
        (None, None, false) => BellandeMeshSync::new_with_metrics(&config, metrics.clone())?,
        _ => return Err(BellandeMeshError::Custom(
            "Invalid certificate configuration. Provide both cert and key paths, or enable dev mode, or neither.".into()
        )),
    };

    // Restore persisted data if available
    if let Some(persistence) = &persistence_manager {
        // Load nodes from persistence
        let nodes = persistence
            .load_nodes()
            .map_err(|e| BellandeMeshError::Custom(format!("Failed to load nodes: {}", e)))?;

        // Restore the nodes structure
        bellande_mesh
            .restore_nodes(nodes.clone())
            .await
            .map_err(|e| BellandeMeshError::Custom(format!("Failed to restore nodes: {}", e)))?;

        // Then load and restore data chunks for each node
        for node in nodes {
            match persistence.load_data_chunks(&node.id) {
                Ok(chunks) => {
                    if let Err(e) = bellande_mesh.restore_data_chunks(node.id, chunks).await {
                        eprintln!(
                            "Error restoring chunks for node {}: {}",
                            node.id.to_hex(),
                            e
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error loading chunks for node {}: {}", node.id.to_hex(), e);
                    continue;
                }
            }
        }
    }

    // Configure metrics collection if enabled
    if let Some(interval) = options.metrics_interval {
        bellande_mesh.start_metrics_collection(interval).await?;
    }

    // Set connection limits if specified
    if let Some(max_conn) = options.max_connections {
        bellande_mesh.set_max_connections(max_conn).await?;
    }

    Ok(bellande_mesh)
}

/// Start the BellandeMeshSync System
pub async fn start(bellande_mesh: &BellandeMeshSync) -> Result<(), BellandeMeshError> {
    bellande_mesh.start().await
}

/// Stop the BellandeMeshSync System
pub async fn stop(bellande_mesh: &BellandeMeshSync) -> Result<(), BellandeMeshError> {
    bellande_mesh.stop().await
}

// Message and Data Handling
pub async fn broadcast(
    bellande_mesh: &BellandeMeshSync,
    data: Vec<u8>,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.broadcast_data(data).await
}

pub async fn send_to_node(
    bellande_mesh: &BellandeMeshSync,
    node_id: NodeId,
    data: Vec<u8>,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.send_data_to_node(node_id, data).await
}

// Node Management
pub async fn get_nodes(bellande_mesh: &BellandeMeshSync) -> Result<Vec<Node>, BellandeMeshError> {
    bellande_mesh.get_nodes().await
}

pub async fn get_node_port(bellande_mesh: &BellandeMeshSync) -> Result<u16, BellandeMeshError> {
    bellande_mesh.get_node_port().await
}

pub async fn get_active_nodes(
    bellande_mesh: &BellandeMeshSync,
) -> Result<Vec<Node>, BellandeMeshError> {
    bellande_mesh.get_all_active_nodes().await
}

pub async fn get_nodes_paginated(
    bellande_mesh: &BellandeMeshSync,
    offset: usize,
    limit: usize,
) -> Result<(Vec<Node>, usize), BellandeMeshError> {
    bellande_mesh.get_nodes_paginated(offset, limit).await
}

pub async fn is_node_connected(
    bellande_mesh: &BellandeMeshSync,
    node_id: NodeId,
) -> Result<bool, BellandeMeshError> {
    bellande_mesh.is_node_connected(&node_id).await
}

pub async fn restore_nodes(
    bellande_mesh: &BellandeMeshSync,
    nodes: Vec<Node>,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.restore_nodes(nodes).await
}

// Network Statistics and Monitoring
pub async fn get_stats(
    bellande_mesh: &BellandeMeshSync,
) -> Result<NetworkStats, BellandeMeshError> {
    bellande_mesh.get_network_stats().await
}

pub async fn get_node_count(bellande_mesh: &BellandeMeshSync) -> Result<usize, BellandeMeshError> {
    bellande_mesh.get_node_count().await
}

pub async fn get_node_list(
    bellande_mesh: &BellandeMeshSync,
) -> Result<Vec<NodeId>, BellandeMeshError> {
    bellande_mesh.get_node_list().await
}

// Maintenance Operations
pub async fn start_metrics_collection(
    bellande_mesh: &BellandeMeshSync,
    interval: u64,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.start_metrics_collection(interval).await
}

pub async fn set_max_connections(
    bellande_mesh: &BellandeMeshSync,
    max_conn: usize,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.set_max_connections(max_conn).await
}

pub async fn cleanup_dead_nodes(bellande_mesh: &BellandeMeshSync) -> Result<(), BellandeMeshError> {
    bellande_mesh.cleanup_dead_nodes().await
}

// Node Discovery and Communication
pub async fn find_closest_nodes(
    bellande_mesh: &BellandeMeshSync,
    target: &NodeId,
    count: usize,
) -> Result<Vec<Node>, BellandeMeshError> {
    Ok(bellande_mesh.find_closest_nodes(target, count).await)
}

pub async fn broadcast_new_node(
    bellande_mesh: &BellandeMeshSync,
    new_node: &Node,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.broadcast_new_node(new_node).await
}

// Protocol Functions
pub async fn handle_join_request(
    bellande_mesh: &BellandeMeshSync,
    data: Vec<u8>,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.handle_join_request(data).await
}

// Utility Functions
pub async fn get_local_id(bellande_mesh: &BellandeMeshSync) -> Result<NodeId, BellandeMeshError> {
    bellande_mesh.get_local_id().await
}

pub async fn get_status(bellande_mesh: &BellandeMeshSync) -> String {
    bellande_mesh.get_status().await
}

pub async fn is_running(bellande_mesh: &BellandeMeshSync) -> Result<bool, BellandeMeshError> {
    bellande_mesh.is_running().await
}
