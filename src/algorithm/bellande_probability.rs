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

use crate::algorithm::connections::BellandeArchError;
use bellande_probability::make_bellande_probability_request;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for spatial probability calculations using Bellande Probability algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialProbabilityConfig {
    pub mu_function: String,
    pub sigma_function: String,
    pub input_vector: Vec<f64>,
    pub dimensions: i32,
    pub full_auth: bool,
    pub use_local_executable: bool,
}

/// Result of a spatial probability calculation using Bellande Probability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilityCalculationResult {
    pub probability_values: Vec<f64>,
    pub confidence_intervals: Option<Vec<Vec<f64>>>,
    pub execution_time_ms: Option<i32>,
    pub statistical_metrics: Option<Value>,
}

/// Extended configuration for advanced Bellande Probability calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedProbabilityConfig {
    pub basic_config: SpatialProbabilityConfig,
    pub distribution_parameters: Option<Value>,
    pub confidence_level: Option<f64>,
    pub prior_probabilities: Option<Vec<f64>>,
}

/// Main spatial probability module that leverages the Bellande Probability algorithm
pub struct SpatialProbabilityCalculator;

impl SpatialProbabilityCalculator {
    /// Performs a spatial probability calculation using the Bellande Probability algorithm
    pub async fn calculate(
        config: SpatialProbabilityConfig,
    ) -> Result<ProbabilityCalculationResult, BellandeArchError> {
        // Validate input parameters
        if config.mu_function.is_empty() || config.sigma_function.is_empty() {
            return Err(BellandeArchError::InvalidParameters(
                "Mu and Sigma functions cannot be empty".to_string(),
            ));
        }

        if config.input_vector.is_empty() {
            return Err(BellandeArchError::InvalidParameters(
                "Input vector cannot be empty".to_string(),
            ));
        }

        if config.dimensions <= 0 {
            return Err(BellandeArchError::InvalidParameters(
                "Dimensions must be a positive integer".to_string(),
            ));
        }

        // Convert input vector to JSON Value
        let x = serde_json::json!(config.input_vector);

        // Run the Bellande Probability algorithm using the API
        let result = if !config.use_local_executable {
            make_bellande_probability_request(
                config.mu_function.clone(),
                config.sigma_function.clone(),
                x,
                config.dimensions,
                config.full_auth,
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

    /// Process the raw Bellande Probability result into our domain-specific format
    pub fn process_result(
        result: Value,
    ) -> Result<ProbabilityCalculationResult, BellandeArchError> {
        // Extract the probability values from the result
        let probability_values = result
            .get("probability_values")
            .and_then(|p| p.as_array())
            .ok_or_else(|| {
                BellandeArchError::AlgorithmError(
                    "Missing probability values in algorithm result".to_string(),
                )
            })?
            .iter()
            .map(|v| {
                v.as_f64().ok_or_else(|| {
                    BellandeArchError::AlgorithmError(
                        "Non-numeric probability value in algorithm result".to_string(),
                    )
                })
            })
            .collect::<Result<Vec<f64>, _>>()?;

        // Extract confidence intervals if available
        let confidence_intervals = result
            .get("confidence_intervals")
            .and_then(|ci| ci.as_array())
            .map(|intervals| {
                intervals
                    .iter()
                    .filter_map(|interval| {
                        interval.as_array().map(|bounds| {
                            bounds
                                .iter()
                                .filter_map(|v| v.as_f64())
                                .collect::<Vec<f64>>()
                        })
                    })
                    .collect::<Vec<Vec<f64>>>()
            });

        // Extract execution time if available
        let execution_time_ms = result
            .get("execution_time_ms")
            .and_then(|t| t.as_i64())
            .map(|t| t as i32);

        // Extract statistical metrics if available
        let statistical_metrics = result.get("statistical_metrics").cloned();

        Ok(ProbabilityCalculationResult {
            probability_values,
            confidence_intervals,
            execution_time_ms,
            statistical_metrics,
        })
    }

    /// Calculates the probability distribution for a given set of inputs
    pub async fn calculate_distribution(
        mu_function: String,
        sigma_function: String,
        inputs: Vec<Vec<f64>>,
        dimensions: i32,
    ) -> Result<Vec<f64>, BellandeArchError> {
        // Validate inputs
        if inputs.is_empty() {
            return Err(BellandeArchError::InvalidParameters(
                "Input vectors cannot be empty".to_string(),
            ));
        }

        if mu_function.is_empty() || sigma_function.is_empty() {
            return Err(BellandeArchError::InvalidParameters(
                "Mu and Sigma functions cannot be empty".to_string(),
            ));
        }

        // Calculate probabilities for each input vector
        let mut probabilities = Vec::new();
        for input_vector in inputs {
            // Basic configuration
            let config = SpatialProbabilityConfig {
                mu_function: mu_function.clone(),
                sigma_function: sigma_function.clone(),
                input_vector,
                dimensions,
                full_auth: false,
                use_local_executable: false,
            };

            // Perform calculation
            let result = Self::calculate(config).await?;

            // Add the first probability value (assuming it's the main one)
            if !result.probability_values.is_empty() {
                probabilities.push(result.probability_values[0]);
            } else {
                return Err(BellandeArchError::AlgorithmError(
                    "Empty probability values returned".to_string(),
                ));
            }
        }

        Ok(probabilities)
    }

    /// Batch processes multiple probability calculations in parallel
    pub async fn batch_calculate(
        configs: Vec<SpatialProbabilityConfig>,
    ) -> Vec<Result<ProbabilityCalculationResult, BellandeArchError>> {
        // Process each configuration in parallel
        let futures = configs.into_iter().map(|config| Self::calculate(config));

        // Collect all results
        join_all(futures).await
    }

    /// Advanced batch processing with custom parameters for each calculation
    pub async fn advanced_batch_calculate(
        configs: Vec<AdvancedProbabilityConfig>,
    ) -> Vec<Result<ProbabilityCalculationResult, BellandeArchError>> {
        let futures = configs.into_iter().map(|config| {
            // Use basic calculation
            Self::calculate(config.basic_config)
        });

        join_all(futures).await
    }

    /// Normalizes a set of probability values to ensure they sum to 1.0
    pub fn normalize_probabilities(probabilities: &[f64]) -> Result<Vec<f64>, BellandeArchError> {
        if probabilities.is_empty() {
            return Err(BellandeArchError::InvalidParameters(
                "Probability vector cannot be empty for normalization".to_string(),
            ));
        }

        // Calculate sum of all probabilities
        let sum = probabilities.iter().sum::<f64>();

        // Check if sum is close to zero to avoid division by zero
        if sum.abs() < 1e-10 {
            return Err(BellandeArchError::AlgorithmError(
                "Cannot normalize probabilities with sum close to zero".to_string(),
            ));
        }

        // Normalize each probability by dividing by the sum
        let normalized = probabilities.iter().map(|&p| p / sum).collect();

        Ok(normalized)
    }

    /// Calculates the weighted average of vectors using probability weights
    pub fn calculate_weighted_average(
        vectors: &[Vec<f64>],
        probabilities: &[f64],
    ) -> Result<Vec<f64>, BellandeArchError> {
        if vectors.is_empty() || probabilities.is_empty() {
            return Err(BellandeArchError::InvalidParameters(
                "Vectors and probabilities cannot be empty for weighted average".to_string(),
            ));
        }

        if vectors.len() != probabilities.len() {
            return Err(BellandeArchError::DimensionMismatch(format!(
                "Mismatch between number of vectors ({}) and probabilities ({})",
                vectors.len(),
                probabilities.len()
            )));
        }

        // Ensure all vectors have the same dimension
        let dimension = vectors[0].len();
        for (i, vector) in vectors.iter().enumerate() {
            if vector.len() != dimension {
                return Err(BellandeArchError::DimensionMismatch(format!(
                    "Vector at index {} has dimension {} but expected {}",
                    i,
                    vector.len(),
                    dimension
                )));
            }
        }

        // Calculate the weighted average for each dimension
        let mut weighted_average = vec![0.0; dimension];
        let prob_sum = probabilities.iter().sum::<f64>();

        if prob_sum.abs() < 1e-10 {
            return Err(BellandeArchError::AlgorithmError(
                "Sum of probabilities is too close to zero".to_string(),
            ));
        }

        for d in 0..dimension {
            for (i, vector) in vectors.iter().enumerate() {
                weighted_average[d] += vector[d] * probabilities[i] / prob_sum;
            }
        }

        Ok(weighted_average)
    }
}

/// Converts a probability value to a log-likelihood for numerical stability
pub fn probability_to_log_likelihood(probability: f64) -> Result<f64, BellandeArchError> {
    if probability <= 0.0 || probability > 1.0 {
        return Err(BellandeArchError::InvalidParameters(format!(
            "Invalid probability value: {}",
            probability
        )));
    }

    Ok(probability.ln())
}

/// Converts a log-likelihood back to a probability value
pub fn log_likelihood_to_probability(log_likelihood: f64) -> Result<f64, BellandeArchError> {
    if log_likelihood > 0.0 {
        return Err(BellandeArchError::InvalidParameters(format!(
            "Invalid log likelihood value: {}",
            log_likelihood
        )));
    }

    Ok(log_likelihood.exp())
}
