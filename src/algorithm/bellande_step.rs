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
use bellande_step::make_bellande_step_request;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for spatial coordinate transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialTransformConfig {
    pub start_coordinates: Vec<f64>,
    pub end_coordinates: Vec<f64>,
    pub iterations: i32,
    pub use_local_executable: bool,
}

/// Result of a spatial transformation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationResult {
    pub path_coordinates: Vec<Vec<f64>>,
    pub convergence_metrics: Option<Vec<f64>>,
    pub execution_time_ms: Option<i32>,
}

/// Extended configuration for advanced transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedTransformConfig {
    pub basic_config: SpatialTransformConfig,
    pub obstacles: Option<Vec<Vec<f64>>>,
    pub constraint_regions: Option<Vec<Vec<Vec<f64>>>>,
    pub preferred_path_bias: Option<f64>,
    pub smoothing_factor: Option<f64>,
}

/// Configuration for coordinate system operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinateSystemConfig {
    pub source_system: String,
    pub target_system: String,
    pub dimension_map: Option<Vec<usize>>,
}

/// Main spatial transformation module that leverages the Bellande Step algorithm
pub struct SpatialTransformer;

impl SpatialTransformer {
    /// Performs a spatial coordinate transformation using the Bellande Step algorithm
    pub async fn transform(
        config: SpatialTransformConfig,
    ) -> Result<TransformationResult, BellandeArchError> {
        // Validate input coordinates
        if config.start_coordinates.is_empty() || config.end_coordinates.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Coordinate vectors cannot be empty".to_string(),
            ));
        }
        let dimensions = config.start_coordinates.len();

        // Verify that both coordinate sets have the same dimensions
        if dimensions != config.end_coordinates.len() {
            return Err(BellandeArchError::DimensionMismatch(format!(
                "Start coordinates have {} dimensions but end coordinates have {}",
                dimensions,
                config.end_coordinates.len()
            )));
        }
        // Convert coordinates to JSON Values
        let node0 = serde_json::json!(config.start_coordinates);
        let node1 = serde_json::json!(config.end_coordinates);

        // Run the Bellande Step algorithm using the API
        let result = if !config.use_local_executable {
            make_bellande_step_request(node0, node1, config.iterations, dimensions as i32)
                .await
                .map_err(|e| BellandeArchError::AlgorithmError(e.to_string()))?
        } else {
            return Err(BellandeArchError::AlgorithmError(
                "Start and end coordinates cannot be empty".to_string(),
            ));
        };

        // Process and transform the results
        Self::process_result(result)
    }

    /// Process the raw Bellande Step result into our domain-specific format
    pub fn process_result(result: Value) -> Result<TransformationResult, BellandeArchError> {
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

        Ok(TransformationResult {
            path_coordinates,
            convergence_metrics,
            execution_time_ms,
        })
    }

    /// Optimizes a path between two points using advanced parameters
    pub async fn optimize_path(
        start: Vec<f64>,
        end: Vec<f64>,
        obstacles: Vec<Vec<f64>>,
        precision: f64,
    ) -> Result<TransformationResult, BellandeArchError> {
        // Validate inputs
        if start.is_empty() || end.is_empty() {
            return Err(BellandeArchError::InvalidCoordinates(
                "Start and end coordinates cannot be empty".to_string(),
            ));
        }

        if start.len() != end.len() {
            return Err(BellandeArchError::DimensionMismatch(format!(
                "Start coordinates have {} dimensions but end coordinates have {}",
                start.len(),
                end.len()
            )));
        }

        // Determine appropriate iteration count based on precision
        let iterations = match precision {
            p if p < 0.01 => 500,
            p if p < 0.1 => 200,
            _ => 100,
        };

        // Basic configuration
        let config = SpatialTransformConfig {
            start_coordinates: start.clone(),
            end_coordinates: end.clone(),
            iterations,
            use_local_executable: false,
        };

        // Perform initial transformation
        let initial_result = Self::transform(config.clone()).await?;

        // If there are obstacles, we need more sophisticated processing
        if !obstacles.is_empty() {
            return Self::optimize_path_with_obstacles(config, obstacles).await;
        }

        Ok(initial_result)
    }

    /// Advanced path optimization that handles obstacles
    pub async fn optimize_path_with_obstacles(
        config: SpatialTransformConfig,
        obstacles: Vec<Vec<f64>>,
    ) -> Result<TransformationResult, BellandeArchError> {
        // Validate obstacle data
        for obstacle in &obstacles {
            if obstacle.len() != config.start_coordinates.len() {
                return Err(BellandeArchError::DimensionMismatch(format!(
                    "Obstacle has {} dimensions but path has {} dimensions",
                    obstacle.len(),
                    config.start_coordinates.len()
                )));
            }
        }

        // Define a safety radius around obstacles
        let safety_radius = 1.0;

        // Create a list of waypoints, starting with the start point
        let mut waypoints = vec![config.start_coordinates.clone()];

        // For each obstacle, determine if we need to route around it
        for obstacle in &obstacles {
            // Create a waypoint that avoids this obstacle

            // For simplicity, we'll just offset in the first dimension
            let mut waypoint = obstacle.clone();
            waypoint[0] += safety_radius * 2.0;

            // Only add unique waypoints that aren't too close to existing ones
            if !waypoints
                .iter()
                .any(|wp| euclidean_distance(wp, &waypoint).unwrap_or(f64::MAX) < safety_radius)
            {
                waypoints.push(waypoint);
            }
        }

        // Add the end point as final waypoint
        waypoints.push(config.end_coordinates.clone());

        // Now transform between each successive pair of waypoints
        let mut segments = Vec::new();
        for i in 0..waypoints.len() - 1 {
            let segment_config = SpatialTransformConfig {
                start_coordinates: waypoints[i].clone(),
                end_coordinates: waypoints[i + 1].clone(),
                iterations: config.iterations / waypoints.len() as i32,
                use_local_executable: config.use_local_executable,
            };

            let segment_result = Self::transform(segment_config).await?;
            segments.push(segment_result);
        }

        // Combine the segments into a single path
        let mut combined_path = Vec::new();
        let mut combined_metrics = Vec::new();
        let mut total_time = 0;

        for (i, segment) in segments.iter().enumerate() {
            if i == 0 {
                // Include the entire first segment
                combined_path.extend(segment.path_coordinates.clone());
            } else {
                // For subsequent segments, skip the first point to avoid duplicates
                combined_path.extend(segment.path_coordinates[1..].to_vec());
            }

            // Combine metrics if available
            if let Some(metrics) = &segment.convergence_metrics {
                combined_metrics.extend(metrics.clone());
            }

            // Add execution time
            if let Some(time) = segment.execution_time_ms {
                total_time += time;
            }
        }

        Ok(TransformationResult {
            path_coordinates: combined_path,
            convergence_metrics: if combined_metrics.is_empty() {
                None
            } else {
                Some(combined_metrics)
            },
            execution_time_ms: Some(total_time),
        })
    }

    /// Batch processes multiple transformations in parallel
    pub async fn batch_transform(
        configs: Vec<SpatialTransformConfig>,
    ) -> Vec<Result<TransformationResult, BellandeArchError>> {
        // Process each configuration in parallel
        let futures = configs.into_iter().map(|config| Self::transform(config));

        // Collect all results
        join_all(futures).await
    }

    /// Advanced batch processing with custom parameters for each transformation
    pub async fn advanced_batch_transform(
        configs: Vec<AdvancedTransformConfig>,
    ) -> Vec<Result<TransformationResult, BellandeArchError>> {
        let futures = configs.into_iter().map(|config| {
            let basic_config = config.basic_config.clone();

            async move {
                let obstacles = config.obstacles.unwrap_or_default();

                if obstacles.is_empty() {
                    // Use basic transformation if no obstacles
                    Self::transform(basic_config).await
                } else {
                    // Otherwise, use path optimization with obstacles
                    Self::optimize_path_with_obstacles(basic_config, obstacles).await
                }
            }
        });

        join_all(futures).await
    }

    /// Calculates the quality of a transformation path
    pub fn calculate_path_quality(
        path: &[Vec<f64>],
        obstacles: &[Vec<f64>],
        obstacle_radius: f64,
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

        // Combine factors into an overall quality score (higher is better)
        // We normalize the path length by dividing by the number of segments
        let normalized_path_length = path_length / (path.len() - 1) as f64;
        let normalized_smoothness = smoothness_penalty / (path.len() - 2).max(1) as f64;

        // Quality formula: We want shorter paths, smoother paths, and paths that avoid obstacles
        let quality =
            10.0 - normalized_path_length - normalized_smoothness * 2.0 - obstacle_penalty * 5.0;

        Ok(quality.max(0.0)) // Ensure quality is non-negative
    }
}

/// Converts between different coordinate representations
pub fn convert_coordinate_system(
    coordinates: &[f64],
    from_system: &str,
    to_system: &str,
) -> Result<Vec<f64>, BellandeArchError> {
    match (from_system, to_system) {
        ("cartesian", "cartesian") => Ok(coordinates.to_vec()),

        ("cartesian", "polar") => {
            if coordinates.len() != 2 {
                return Err(BellandeArchError::DimensionMismatch(
                    "Cartesian to polar conversion requires exactly 2 dimensions".to_string(),
                ));
            }

            let x = coordinates[0];
            let y = coordinates[1];
            let r = (x.powi(2) + y.powi(2)).sqrt();
            let theta = y.atan2(x);

            Ok(vec![r, theta])
        }

        ("polar", "cartesian") => {
            if coordinates.len() != 2 {
                return Err(BellandeArchError::DimensionMismatch(
                    "Polar to Cartesian conversion requires exactly 2 dimensions".to_string(),
                ));
            }

            let r = coordinates[0];
            let theta = coordinates[1];
            let x = r * theta.cos();
            let y = r * theta.sin();

            Ok(vec![x, y])
        }

        ("cartesian", "cylindrical") => {
            if coordinates.len() != 3 {
                return Err(BellandeArchError::DimensionMismatch(
                    "Cartesian to cylindrical conversion requires exactly 3 dimensions".to_string(),
                ));
            }

            let x = coordinates[0];
            let y = coordinates[1];
            let z = coordinates[2];

            let rho = (x.powi(2) + y.powi(2)).sqrt();
            let phi = y.atan2(x);

            Ok(vec![rho, phi, z])
        }

        ("cylindrical", "cartesian") => {
            if coordinates.len() != 3 {
                return Err(BellandeArchError::DimensionMismatch(
                    "Cylindrical to Cartesian conversion requires exactly 3 dimensions".to_string(),
                ));
            }

            let rho = coordinates[0];
            let phi = coordinates[1];
            let z = coordinates[2];

            let x = rho * phi.cos();
            let y = rho * phi.sin();

            Ok(vec![x, y, z])
        }

        ("cartesian", "spherical") => {
            if coordinates.len() != 3 {
                return Err(BellandeArchError::DimensionMismatch(
                    "Cartesian to spherical conversion requires exactly 3 dimensions".to_string(),
                ));
            }

            let x = coordinates[0];
            let y = coordinates[1];
            let z = coordinates[2];

            let r = (x.powi(2) + y.powi(2) + z.powi(2)).sqrt();
            let theta = if r > 0.0 { (z / r).acos() } else { 0.0 };
            let phi = y.atan2(x);

            Ok(vec![r, theta, phi])
        }

        ("spherical", "cartesian") => {
            if coordinates.len() != 3 {
                return Err(BellandeArchError::DimensionMismatch(
                    "Spherical to Cartesian conversion requires exactly 3 dimensions".to_string(),
                ));
            }

            let r = coordinates[0];
            let theta = coordinates[1];
            let phi = coordinates[2];

            let x = r * theta.sin() * phi.cos();
            let y = r * theta.sin() * phi.sin();
            let z = r * theta.cos();

            Ok(vec![x, y, z])
        }

        _ => Err(BellandeArchError::InvalidCoordinates(format!(
            "Unsupported coordinate system conversion: {} to {}",
            from_system, to_system
        ))),
    }
}

/// Interpolates between two points to create a sequence of intermediate points
pub fn interpolate_points(
    start: &[f64],
    end: &[f64],
    num_points: usize,
) -> Result<Vec<Vec<f64>>, BellandeArchError> {
    if start.len() != end.len() {
        return Err(BellandeArchError::DimensionMismatch(format!(
            "Points have different dimensions: {} and {}",
            start.len(),
            end.len()
        )));
    }

    if num_points < 2 {
        return Err(BellandeArchError::InvalidCoordinates(
            "Number of points must be at least 2 (start and end)".to_string(),
        ));
    }

    let mut result = Vec::with_capacity(num_points);

    // Add the start point
    result.push(start.to_vec());

    // Add intermediate points
    if num_points > 2 {
        for i in 1..num_points - 1 {
            let t = i as f64 / (num_points - 1) as f64;
            let mut point = Vec::with_capacity(start.len());

            for j in 0..start.len() {
                let value = start[j] + t * (end[j] - start[j]);
                point.push(value);
            }

            result.push(point);
        }
    }

    // Add the end point
    result.push(end.to_vec());

    Ok(result)
}
