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

/// Initialize the BellandeMeshSync System
/// This function takes a configuration and sets up the BellandeMeshSync System.
/// It returns a BellandeMeshSync instance that can be used to interact with the mesh network.
pub async fn init(config: Config) -> Result<BellandeMeshSync, BellandeMeshError> {
    let bellande_mesh = BellandeMeshSync::new(&config)?;
    Ok(bellande_mesh)
}

/// Start the BellandeMeshSync System
/// This function takes a BellandeMeshSync instance and starts the mesh network operations.
pub async fn start(bellande_mesh: &BellandeMeshSync) -> Result<(), BellandeMeshError> {
    bellande_mesh.start().await
}
