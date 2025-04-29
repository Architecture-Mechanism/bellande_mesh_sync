// Copyright (C) 2025 Bellande Architecture Mechanism Research Innovation Center, Ronaldson Bellande

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

use crate::algorithm::connections::{euclidean_distance, BellandeArchError};
use bellande_node_importance::make_bellande_node_importance_request;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for node importance evaluation using Bellande Node Importance algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeImportanceConfig {
    pub node_coordinates: Vec<f64>,
    pub segment_id: String,
    pub recent_nodes: Vec<Vec<f64>>,
    pub important_nodes_by_segment: std::collections::HashMap<String, Vec<Vec<f64>>>,
    pub adjacent_segments: std::collections::HashMap<String, Vec<String>>,
    pub grid_steps: Vec<f64>,
    pub min_segment_coverage: f64,
    pub use_local_executable: bool,
}

/// Result of a node importance evaluation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeImportanceResult {
    pub importance_score: f64,
    pub coverage_metrics: Option<Vec<f64>>,
    pub segment_coverage: Option<f64>,
    pub execution_time_ms: Option<i32>,
    pub node_attributes: Option<std::collections::HashMap<String, Value>>,
}

/// Extended configuration for advanced node importance evaluations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedNodeImportanceConfig {
    pub basic_config: NodeImportanceConfig,
    pub priority_regions: Option<Vec<Vec<Vec<f64>>>>,
    pub coverage_bias: Option<f64>,
    pub weighting_factor: Option<f64>,
}

/// Main node importance evaluation module that leverages the Bellande Node Importance algorithm
pub struct NodeImportanceEvaluator;

impl NodeImportanceEvaluator {
    /// Evaluates the importance of a node using the Bellande Node Importance algorithm
    pub async fn evaluate(
        config: NodeImportanceConfig,
    ) -> Result<NodeImportanceResult, BellandeArchError> {
        // Validate input coordinates
        if config.node_coordinates.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Node coordinates cannot be empty".to_string(),
            ));
        }

        // Validate segment ID
        if config.segment_id.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Segment ID cannot be empty".to_string(),
            ));
        }

        // Validate grid steps
        if config.grid_steps.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Grid steps cannot be empty".to_string(),
            ));
        }

        let dimensions = config.node_coordinates.len();

        // Verify that grid steps have the same dimensions as node coordinates
        if dimensions != config.grid_steps.len() {
            return Err(BellandeArchError::DimensionMismatch(format!(
                "Coordinate dimension mismatch: node={}, grid_steps={}",
                dimensions,
                config.grid_steps.len()
            )));
        }

        // Create the node JSON structure
        let node = serde_json::json!({
            "coords": config.node_coordinates,
            "segment": config.segment_id
        });

        // Convert recent nodes to JSON Value
        let recent_nodes = serde_json::json!(config.recent_nodes);

        // Convert important nodes to JSON Value
        let important_nodes = serde_json::json!(config.important_nodes_by_segment);

        // Convert adjacent segments to JSON Value
        let adjacent_segments = serde_json::json!(config.adjacent_segments);

        // Convert grid steps to JSON Value
        let grid_steps = serde_json::json!(config.grid_steps);

        // Run the Bellande Node Importance algorithm using the API
        let result = if !config.use_local_executable {
            make_bellande_node_importance_request(
                node,
                recent_nodes,
                important_nodes,
                adjacent_segments,
                grid_steps,
                config.min_segment_coverage,
            )
            .await
            .map_err(|e| BellandeArchError::AlgorithmError(e.to_string()))?
        } else {
            return Err(BellandeArchError::AlgorithmError(
                "Local executable functionality not implemented".to_string(),
            ));
        };

        // Process and transform the results
        Self::process_result(result)
    }

    /// Process the raw Bellande Node Importance result into our domain-specific format
    pub fn process_result(result: Value) -> Result<NodeImportanceResult, BellandeArchError> {
        // Extract the importance score from the result
        let importance_score = result
            .get("importance_score")
            .and_then(|s| s.as_f64())
            .ok_or_else(|| {
                BellandeArchError::AlgorithmError(
                    "Missing importance score in algorithm result".to_string(),
                )
            })?;

        // Extract coverage metrics if available
        let coverage_metrics = result
            .get("coverage_metrics")
            .and_then(|m| m.as_array())
            .map(|metrics| {
                metrics
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .collect::<Vec<f64>>()
            });

        // Extract segment coverage if available
        let segment_coverage = result.get("segment_coverage").and_then(|c| c.as_f64());

        // Extract execution time if available
        let execution_time_ms = result
            .get("execution_time_ms")
            .and_then(|t| t.as_i64())
            .map(|t| t as i32);

        // Extract node attributes if available
        let node_attributes = result
            .get("node_attributes")
            .and_then(|a| a.as_object())
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<std::collections::HashMap<String, Value>>()
            });

        Ok(NodeImportanceResult {
            importance_score,
            coverage_metrics,
            segment_coverage,
            execution_time_ms,
            node_attributes,
        })
    }

    /// Evaluates node importance with optimal parameters based on segment characteristics
    pub async fn evaluate_optimal(
        node_coords: Vec<f64>,
        segment_id: String,
        recent_nodes: Vec<Vec<f64>>,
        important_nodes_by_segment: std::collections::HashMap<String, Vec<Vec<f64>>>,
        adjacent_segments: std::collections::HashMap<String, Vec<String>>,
    ) -> Result<NodeImportanceResult, BellandeArchError> {
        // Validate inputs
        if node_coords.is_empty() || segment_id.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Node coordinates and segment ID cannot be empty".to_string(),
            ));
        }

        let dimensions = node_coords.len();

        // Determine appropriate grid steps based on segment characteristics
        let grid_steps = if let Some(segment_nodes) = important_nodes_by_segment.get(&segment_id) {
            if !segment_nodes.is_empty() {
                // Calculate average distance between nodes in this segment
                let mut total_distance = 0.0;
                let mut count = 0;

                for i in 0..segment_nodes.len() {
                    for j in i + 1..segment_nodes.len() {
                        if let Ok(dist) = euclidean_distance(&segment_nodes[i], &segment_nodes[j]) {
                            total_distance += dist;
                            count += 1;
                        }
                    }
                }

                let avg_distance = if count > 0 {
                    total_distance / count as f64
                } else {
                    1.0
                };
                // Use 10% of average distance as grid step
                vec![avg_distance * 0.1; dimensions]
            } else {
                // Default grid steps if no nodes in segment
                vec![1.0; dimensions]
            }
        } else {
            // Default grid steps if segment not found
            vec![1.0; dimensions]
        };

        // Determine minimum segment coverage based on adjacent segments count
        let adjacent_count = adjacent_segments
            .get(&segment_id)
            .map(|adj| adj.len())
            .unwrap_or(0);

        let min_segment_coverage = match adjacent_count {
            0 => 0.7,     // Isolated segment, require higher coverage
            1..=2 => 0.6, // Few connections
            3..=5 => 0.5, // Moderate connections
            _ => 0.4,     // Many connections, can be more flexible
        };

        // Basic configuration
        let config = NodeImportanceConfig {
            node_coordinates: node_coords,
            segment_id,
            recent_nodes,
            important_nodes_by_segment,
            adjacent_segments,
            grid_steps,
            min_segment_coverage,
            use_local_executable: false,
        };

        // Perform evaluation
        Self::evaluate(config).await
    }

    /// Batch processes multiple node importance evaluations in parallel
    pub async fn batch_evaluate(
        configs: Vec<NodeImportanceConfig>,
    ) -> Vec<Result<NodeImportanceResult, BellandeArchError>> {
        // Process each configuration in parallel
        let futures = configs.into_iter().map(|config| Self::evaluate(config));

        // Collect all results
        join_all(futures).await
    }

    /// Advanced batch processing with custom parameters for each evaluation
    pub async fn advanced_batch_evaluate(
        configs: Vec<AdvancedNodeImportanceConfig>,
    ) -> Vec<Result<NodeImportanceResult, BellandeArchError>> {
        let futures = configs.into_iter().map(|config| {
            // Use basic evaluation
            Self::evaluate(config.basic_config)
        });

        join_all(futures).await
    }

    /// Calculates a comparative importance score between two nodes
    pub fn calculate_comparative_importance(
        node1_result: &NodeImportanceResult,
        node2_result: &NodeImportanceResult,
        weighting_factor: f64,
    ) -> f64 {
        // Basic comparison of importance scores
        let importance_diff = node1_result.importance_score - node2_result.importance_score;

        // Consider segment coverage if available
        let coverage_factor = match (node1_result.segment_coverage, node2_result.segment_coverage) {
            (Some(cov1), Some(cov2)) => (cov1 - cov2) * weighting_factor,
            _ => 0.0,
        };

        // Combined score
        importance_diff + coverage_factor
    }
}
