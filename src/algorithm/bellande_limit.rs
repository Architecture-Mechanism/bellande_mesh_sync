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

use crate::algorithm::connections::{
    euclidean_distance, vector_dot_product, vector_length, vector_subtract, BellandeArchError,
};
use bellande_limit::make_bellande_limit_request;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for spatial coordinate transformation using Bellande Limit algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialLimitConfig {
    pub start_coordinates: Vec<f64>,
    pub end_coordinates: Vec<f64>,
    pub environment_dimensions: Vec<f64>,
    pub step_sizes: Vec<f64>,
    pub goal_coordinates: Vec<f64>,
    pub obstacles: Option<Vec<Vec<f64>>>,
    pub search_radius: Option<f64>,
    pub sample_points: Option<i32>,
    pub use_local_executable: bool,
}

/// Result of a spatial transformation operation using Bellande Limit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitTransformationResult {
    pub path_coordinates: Vec<Vec<f64>>,
    pub convergence_metrics: Option<Vec<f64>>,
    pub execution_time_ms: Option<i32>,
    pub path_quality: Option<f64>,
}

/// Extended configuration for advanced Bellande Limit transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedLimitConfig {
    pub basic_config: SpatialLimitConfig,
    pub constraint_regions: Option<Vec<Vec<Vec<f64>>>>,
    pub preferred_path_bias: Option<f64>,
    pub smoothing_factor: Option<f64>,
}

/// Main spatial transformation module that leverages the Bellande Limit algorithm
pub struct SpatialLimitTransformer;

impl SpatialLimitTransformer {
    /// Performs a spatial coordinate transformation using the Bellande Limit algorithm
    pub async fn transform(
        config: SpatialLimitConfig,
    ) -> Result<LimitTransformationResult, BellandeArchError> {
        // Validate input coordinates
        if config.start_coordinates.is_empty() || config.end_coordinates.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Coordinate vectors cannot be empty".to_string(),
            ));
        }

        // Validate environment dimensions
        if config.environment_dimensions.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Environment dimensions cannot be empty".to_string(),
            ));
        }

        let dimensions = config.start_coordinates.len();

        // Verify that all coordinate sets have the same dimensions
        if dimensions != config.end_coordinates.len()
            || dimensions != config.environment_dimensions.len()
            || dimensions != config.step_sizes.len()
            || dimensions != config.goal_coordinates.len()
        {
            return Err(BellandeArchError::DimensionMismatch(format!(
                "Coordinate dimension mismatch: start={}, end={}, environment={}, size={}, goal={}",
                dimensions,
                config.end_coordinates.len(),
                config.environment_dimensions.len(),
                config.step_sizes.len(),
                config.goal_coordinates.len()
            )));
        }

        // Convert coordinates to JSON Values
        let node0 = serde_json::json!(config.start_coordinates);
        let node1 = serde_json::json!(config.end_coordinates);
        let environment = serde_json::json!(config.environment_dimensions);
        let size = serde_json::json!(config.step_sizes);
        let goal = serde_json::json!(config.goal_coordinates);
        let obstacles = config.obstacles.as_ref().map(|o| serde_json::json!(o));

        // Set default values if not provided
        let search_radius = config.search_radius.unwrap_or(50.0);
        let sample_points = config.sample_points.unwrap_or(20);

        // Run the Bellande Limit algorithm using the API
        let result = if !config.use_local_executable {
            make_bellande_limit_request(
                node0,
                node1,
                environment,
                size,
                goal,
                obstacles,
                search_radius,
                sample_points,
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

    /// Process the raw Bellande Limit result into our domain-specific format
    pub fn process_result(result: Value) -> Result<LimitTransformationResult, BellandeArchError> {
        // Extract the path data from the result
        let path = result
            .get("path")
            .and_then(|p| p.as_array())
            .ok_or_else(|| {
                BellandeArchError::AlgorithmError(
                    "Missing path data in algorithm result".to_string(),
                )
            })?;

        // Convert the path to our format
        let mut path_coordinates = Vec::new();
        for point in path {
            let coords = point
                .as_array()
                .ok_or_else(|| {
                    BellandeArchError::AlgorithmError(
                        "Invalid point format in algorithm result".to_string(),
                    )
                })?
                .iter()
                .map(|v| {
                    v.as_f64().ok_or_else(|| {
                        BellandeArchError::AlgorithmError(
                            "Non-numeric coordinate in algorithm result".to_string(),
                        )
                    })
                })
                .collect::<Result<Vec<f64>, _>>()?;

            path_coordinates.push(coords);
        }

        // Extract any metrics if available
        let convergence_metrics = result
            .get("convergence_metrics")
            .and_then(|m| m.as_array())
            .map(|metrics| {
                metrics
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .collect::<Vec<f64>>()
            });

        // Extract execution time if available
        let execution_time_ms = result
            .get("execution_time_ms")
            .and_then(|t| t.as_i64())
            .map(|t| t as i32);

        // Extract path quality if available
        let path_quality = result.get("path_quality").and_then(|q| q.as_f64());

        Ok(LimitTransformationResult {
            path_coordinates,
            convergence_metrics,
            execution_time_ms,
            path_quality,
        })
    }

    /// Optimizes a path between two points considering environment constraints
    pub async fn optimize_path(
        start: Vec<f64>,
        end: Vec<f64>,
        environment: Vec<f64>,
        obstacles: Vec<Vec<f64>>,
        precision: f64,
    ) -> Result<LimitTransformationResult, BellandeArchError> {
        // Validate inputs
        if start.is_empty() || end.is_empty() || environment.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Start, end, and environment coordinates cannot be empty".to_string(),
            ));
        }

        if start.len() != end.len() || start.len() != environment.len() {
            return Err(BellandeArchError::DimensionMismatch(format!(
                "Dimension mismatch: start={}, end={}, environment={}",
                start.len(),
                end.len(),
                environment.len()
            )));
        }

        // Determine appropriate step sizes based on precision
        let step_sizes = environment
            .iter()
            .map(|&dim| dim * precision / 100.0)
            .collect::<Vec<f64>>();

        // Use end coordinates as goal initially
        let goal_coordinates = end.clone();

        // Sample points based on precision
        let sample_points = match precision {
            p if p < 0.01 => 50,
            p if p < 0.1 => 30,
            _ => 20,
        };

        // Search radius based on environment size
        let search_radius = environment.iter().sum::<f64>() / environment.len() as f64 * 0.1;

        // Basic configuration
        let config = SpatialLimitConfig {
            start_coordinates: start.clone(),
            end_coordinates: end.clone(),
            environment_dimensions: environment,
            step_sizes,
            goal_coordinates,
            obstacles: Some(obstacles),
            search_radius: Some(search_radius),
            sample_points: Some(sample_points),
            use_local_executable: false,
        };

        // Perform transformation
        Self::transform(config).await
    }

    /// Batch processes multiple transformations in parallel
    pub async fn batch_transform(
        configs: Vec<SpatialLimitConfig>,
    ) -> Vec<Result<LimitTransformationResult, BellandeArchError>> {
        // Process each configuration in parallel
        let futures = configs.into_iter().map(|config| Self::transform(config));

        // Collect all results
        join_all(futures).await
    }

    /// Advanced batch processing with custom parameters for each transformation
    pub async fn advanced_batch_transform(
        configs: Vec<AdvancedLimitConfig>,
    ) -> Vec<Result<LimitTransformationResult, BellandeArchError>> {
        let futures = configs.into_iter().map(|config| {
            // Use basic transformation
            Self::transform(config.basic_config)
        });

        join_all(futures).await
    }

    /// Calculates the quality of a transformation path
    pub fn calculate_path_quality(
        path: &[Vec<f64>],
        obstacles: &[Vec<f64>],
        obstacle_radius: f64,
        environment_dimensions: &[f64],
    ) -> Result<f64, BellandeArchError> {
        if path.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Path cannot be empty for quality calculation".to_string(),
            ));
        }

        // Calculate path length
        let mut path_length = 0.0;
        for i in 0..path.len() - 1 {
            path_length += euclidean_distance(&path[i], &path[i + 1])?;
        }

        // Calculate minimum distance to obstacles
        let mut min_obstacle_distance = f64::MAX;
        if !obstacles.is_empty() {
            for point in path {
                for obstacle in obstacles {
                    let distance = euclidean_distance(point, obstacle)?;
                    if distance < min_obstacle_distance {
                        min_obstacle_distance = distance;
                    }
                }
            }
        } else {
            min_obstacle_distance = f64::MAX; // No obstacles, so distance is "infinite"
        }

        // Calculate path smoothness (using angle between segments)
        let mut smoothness_penalty = 0.0;
        if path.len() >= 3 {
            for i in 0..path.len() - 2 {
                let v1 = vector_subtract(&path[i + 1], &path[i])?;
                let v2 = vector_subtract(&path[i + 2], &path[i + 1])?;

                let dot_product = vector_dot_product(&v1, &v2)?;
                let v1_len = vector_length(&v1)?;
                let v2_len = vector_length(&v2)?;

                if v1_len > 0.0 && v2_len > 0.0 {
                    let cos_angle = dot_product / (v1_len * v2_len);
                    let angle = cos_angle.acos();

                    // Penalty increases with sharper turns
                    smoothness_penalty += angle;
                }
            }
        }

        // Obstacle proximity penalty (increases as we get closer to obstacle_radius)
        let obstacle_penalty = if min_obstacle_distance < obstacle_radius * 2.0 {
            (obstacle_radius * 2.0 - min_obstacle_distance) / obstacle_radius
        } else {
            0.0
        };

        // Environment utilization factor - how well the path uses available space
        let env_size = environment_dimensions.iter().product::<f64>();
        let path_box_size = calculate_path_box_size(path)?;
        let utilization_factor = path_box_size / env_size;

        // Combine factors into an overall quality score (higher is better)
        // We normalize the path length by dividing by the number of segments
        let normalized_path_length = path_length / (path.len() - 1) as f64;
        let normalized_smoothness = smoothness_penalty / (path.len() - 2).max(1) as f64;

        // Quality formula: We want shorter paths, smoother paths, paths that avoid obstacles, and good use of space
        let quality =
            10.0 - normalized_path_length - normalized_smoothness * 2.0 - obstacle_penalty * 5.0
                + utilization_factor * 2.0;

        Ok(quality.max(0.0)) // Ensure quality is non-negative
    }
}

/// Calculates the box size that encompasses the entire path
fn calculate_path_box_size(path: &[Vec<f64>]) -> Result<f64, BellandeArchError> {
    if path.is_empty() {
        return Err(BellandeArchError::InvalidCoordinates(
            "Path cannot be empty for box size calculation".to_string(),
        ));
    }

    let dimensions = path[0].len();

    // Find min and max for each dimension
    let mut min_coords = vec![f64::MAX; dimensions];
    let mut max_coords = vec![f64::MIN; dimensions];

    for point in path {
        if point.len() != dimensions {
            return Err(BellandeArchError::DimensionMismatch(
                "Not all points in path have the same dimensions".to_string(),
            ));
        }

        for (i, &value) in point.iter().enumerate() {
            min_coords[i] = min_coords[i].min(value);
            max_coords[i] = max_coords[i].max(value);
        }
    }

    // Calculate box size (volume or area)
    let mut size = 1.0;
    for i in 0..dimensions {
        size *= max_coords[i] - min_coords[i];
    }

    Ok(size)
}
