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

use std::fmt;

/// Error types specific to this library
#[derive(Debug)]
pub enum BellandeArchError {
    DimensionMismatch(String),
    InvalidParameters(String),
    InvalidCoordinates(String),
    AlgorithmError(String),
    NetworkError(String),
}

impl fmt::Display for BellandeArchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BellandeArchError::DimensionMismatch(msg) => write!(f, "Dimension mismatch: {}", msg),
            BellandeArchError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            BellandeArchError::InvalidCoordinates(msg) => write!(f, "Invalid coordinates: {}", msg),
            BellandeArchError::AlgorithmError(msg) => write!(f, "Algorithm error: {}", msg),
            BellandeArchError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

/// Subtracts one vector from another
pub fn vector_subtract(a: &[f64], b: &[f64]) -> Result<Vec<f64>, BellandeArchError> {
    if a.len() != b.len() {
        return Err(BellandeArchError::DimensionMismatch(format!(
            "Cannot subtract vectors with different dimensions: {} and {}",
            a.len(),
            b.len()
        )));
    }

    Ok(a.iter().zip(b.iter()).map(|(x, y)| x - y).collect())
}

/// Calculates the dot product of two vectors
pub fn vector_dot_product(a: &[f64], b: &[f64]) -> Result<f64, BellandeArchError> {
    if a.len() != b.len() {
        return Err(BellandeArchError::DimensionMismatch(format!(
            "Cannot calculate dot product of vectors with different dimensions: {} and {}",
            a.len(),
            b.len()
        )));
    }

    Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
}

/// Calculates the Euclidean length of a vector
pub fn vector_length(v: &[f64]) -> Result<f64, BellandeArchError> {
    Ok(v.iter().map(|x| x * x).sum::<f64>().sqrt())
}

/// Calculates the Euclidean distance between two points
pub fn euclidean_distance(point1: &[f64], point2: &[f64]) -> Result<f64, BellandeArchError> {
    if point1.len() != point2.len() {
        return Err(BellandeArchError::DimensionMismatch(format!(
            "Points have different dimensions: {} and {}",
            point1.len(),
            point2.len()
        )));
    }

    let sum_squared: f64 = point1
        .iter()
        .zip(point2.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();

    Ok(sum_squared.sqrt())
}

/// Validates if a point is within the bounds of a given space
pub fn is_point_valid(point: &[f64], min_bounds: &[f64], max_bounds: &[f64]) -> bool {
    if point.len() != min_bounds.len() || point.len() != max_bounds.len() {
        return false;
    }

    point
        .iter()
        .zip(min_bounds.iter().zip(max_bounds.iter()))
        .all(|(p, (min, max))| p >= min && p <= max)
}
