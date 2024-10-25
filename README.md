# Bellande Mesh Sync

- a comprehensive data synchronization system

# Features
**Full Protocol Support:**
- TCP/UDP handling
- HTTP/HTTPS servers
- TLS encryption
- Binary message protocol

**Node Management:**
- Node discovery
- Dead node cleanup
- Peer synchronization
- Data chunk handling

**Message Handling:**
- Asynchronous message processing
- Multiple message types
- Error handling
- Rate limiting

**Monitoring:**
- Network statistics
- Status reporting
- Error logging
- Performance metrics

**Security:**
- TLS encryption
- Token validation
- Node authentication

# Usage 
```
use bellande_mesh_sync::{init, init_with_options, start, stop, MeshOptions, Config};

async fn example() -> Result<(), BellandeMeshError> {
    // Basic initialization
    let config = Config {
        listen_address: "127.0.0.1:8000".to_string(),
        node_timeout: 300,
    };
    let mesh = init(config.clone()).await?;
    start(&mesh).await?;

    // Or with custom options
    let options = MeshOptions {
        dev_mode: true,
        metrics_interval: Some(30),
        enable_persistence: true,
        ..Default::default()
    };
    let mesh = init_with_options(config, options).await?;
    start(&mesh).await?;

    // Use other functionalities
    broadcast(&mesh, b"Hello network!".to_vec()).await?;
    let stats = get_stats(&mesh).await?;
    let nodes = get_nodes(&mesh).await?;

    stop(&mesh).await?;
    Ok(())
}
```
## Website Crates
- https://crates.io/crates/bellande_mesh_sync

### Installation
- `cargo add bellande_mesh_sync`

```
Name: bellande_mesh_sync
Summary: Bellande Operating System Comprehensive data synchronization system
Home-page: github.com/Architecture-Mechanism/bellande_mesh_sync
Author: Ronaldson Bellande
Author-email: ronaldsonbellande@gmail.com
License: GNU General Public License v3.0
```

## License
Bellande Mesh Sync is distributed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.en.html), see [LICENSE](https://github.com/Architecture-Mechanism/bellande_mesh_sync/blob/main/LICENSE) and [NOTICE](https://github.com/Architecture-Mechanism/bellande_mesh_sync/blob/main/LICENSE) for more information.
