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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config::Config;
    use crate::mesh::mesh::BellandeMeshSync;
    use tokio::net::TcpStream;
    use uuid::Uuid;

    async fn setup_test_mesh() -> BellandeMeshSync {
        let config = Config {
            db_url: ":memory:".to_string(),
            listen_address: "127.0.0.1:0".to_string(),
            bootstrap_nodes: vec![],
        };
        BellandeMeshSync::new(&config).await.unwrap()
    }

    #[tokio::test]
    async fn test_node_join() {
        let mesh = setup_test_mesh().await;
        let node_id = Uuid::new_v4();
        let public_key = mesh.encryption.public_key();

        // Simulate a node joining
        mesh.handle_join_request(node_id, public_key).await.unwrap();

        let nodes = mesh.nodes.read().await;
        assert!(nodes.iter().any(|node| node.id == node_id));
    }

    #[tokio::test]
    async fn test_data_sync() {
        let mesh = setup_test_mesh().await;
        let node_id = Uuid::new_v4();
        let public_key = mesh.encryption.public_key();

        // Add a node
        mesh.handle_join_request(node_id, public_key).await.unwrap();

        // Create a test data chunk
        let chunk = DataChunk {
            id: Uuid::new_v4(),
            content: vec![1, 2, 3, 4],
            checksum: "test_checksum".to_string(),
            version: 1,
            last_modified: chrono::Utc::now(),
            author: node_id,
            parent_versions: vec![],
        };

        // Sync the data
        mesh.handle_data_sync(node_id, vec![chunk.clone()])
            .await
            .unwrap();

        // Verify the data was synced
        let nodes = mesh.nodes.read().await;
        let node = nodes.iter().find(|n| n.id == node_id).unwrap();
        let node_data = node.data.read().await;
        assert!(node_data.contains_key(&chunk.id));
    }

    #[tokio::test]
    async fn test_conflict_resolution() {
        let mesh = setup_test_mesh().await;
        let node1_id = Uuid::new_v4();
        let node2_id = Uuid::new_v4();
        let public_key = mesh.encryption.public_key();

        // Add two nodes
        mesh.handle_join_request(node1_id, public_key)
            .await
            .unwrap();
        mesh.handle_join_request(node2_id, public_key)
            .await
            .unwrap();

        // Create conflicting data chunks
        let chunk_id = Uuid::new_v4();
        let chunk1 = DataChunk {
            id: chunk_id,
            content: vec![1, 2, 3],
            checksum: "checksum1".to_string(),
            version: 1,
            last_modified: chrono::Utc::now(),
            author: node1_id,
            parent_versions: vec![],
        };
        let chunk2 = DataChunk {
            id: chunk_id,
            content: vec![4, 5, 6],
            checksum: "checksum2".to_string(),
            version: 1,
            last_modified: chrono::Utc::now(),
            author: node2_id,
            parent_versions: vec![],
        };

        // Sync conflicting data
        mesh.handle_data_sync(node1_id, vec![chunk1]).await.unwrap();
        mesh.handle_data_sync(node2_id, vec![chunk2]).await.unwrap();

        // Verify conflict resolution
        let nodes = mesh.nodes.read().await;
        for node in nodes.iter() {
            let node_data = node.data.read().await;
            let resolved_chunk = node_data.get(&chunk_id).unwrap();
            assert_eq!(resolved_chunk.version, 2);
            assert_eq!(resolved_chunk.parent_versions.len(), 2);
        }
    }
}
