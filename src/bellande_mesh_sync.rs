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

mod config;
mod data;
mod dht;
mod encryption;
mod error;
mod mesh;
mod metrics;
mod node;
mod persistence;

pub use crate::config::config::Config;
pub use crate::error::error::BellandeMeshError;
pub use crate::mesh::mesh::BellandeMeshSync;
use crate::mesh::mesh::NetworkStats;
pub use crate::metrics::metrics::MetricsManager;
pub use crate::node::node::{Node, NodeId, PublicKey};
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
        let nodes = persistence.load_nodes()?;
        bellande_mesh.restore_nodes(nodes).await?;

        // Load data chunks for each node
        for node in nodes {
            let chunks = persistence.load_data_chunks(&node.id)?;
            bellande_mesh.restore_data_chunks(node.id, chunks).await?;
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

/// Broadcast a message to all nodes in the network
pub async fn broadcast(
    bellande_mesh: &BellandeMeshSync,
    data: Vec<u8>,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.broadcast_data(data).await
}

/// Get the current network statistics
pub async fn get_stats(
    bellande_mesh: &BellandeMeshSync,
) -> Result<NetworkStats, BellandeMeshError> {
    bellande_mesh.get_network_stats().await
}

/// Get the list of currently connected nodes
pub async fn get_nodes(bellande_mesh: &BellandeMeshSync) -> Result<Vec<Node>, BellandeMeshError> {
    bellande_mesh.get_all_nodes().await
}

/// Check if a specific node is connected to the network
pub async fn is_node_connected(
    bellande_mesh: &BellandeMeshSync,
    node_id: NodeId,
) -> Result<bool, BellandeMeshError> {
    bellande_mesh.is_node_connected(&node_id).await
}

/// Send a message to a specific node
pub async fn send_to_node(
    bellande_mesh: &BellandeMeshSync,
    node_id: NodeId,
    data: Vec<u8>,
) -> Result<(), BellandeMeshError> {
    bellande_mesh.send_data_to_node(node_id, data).await
}
