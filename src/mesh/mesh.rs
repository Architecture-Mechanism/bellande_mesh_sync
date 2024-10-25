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

use crate::config::config::Config;
use crate::data::data::DataChunk;
use crate::error::error::BellandeMeshError;
pub use crate::metrics::metrics::MetricsManager;
use crate::node::node::{Message, Node, NodeId, PublicKey};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, StatusCode};
use hyper_rustls::HttpsConnectorBuilder;
use rustls::{Certificate, PrivateKey, ServerConfig};
use serde::Serialize;
use serde_json;
use std::convert::Infallible;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tokio::net::{TcpListener as TokioTcpListener, UdpSocket as TokioUdpSocket};
use tokio::signal;
use tokio::sync::{mpsc, Mutex};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

// Constants
const UDP_BUFFER_SIZE: usize = 65536;
const HTTP_PORT_OFFSET: u16 = 1;
const HTTPS_PORT_OFFSET: u16 = 2;
const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB
const CHANNEL_BUFFER_SIZE: usize = 1000;
const SYNC_INTERVAL: Duration = Duration::from_secs(60);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(300);
const PING_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct NetworkStats {
    tcp_connections: usize,
    udp_packets_received: usize,
    http_requests: usize,
    https_requests: usize,
    active_nodes: usize,
    total_messages: usize,
    start_time: SystemTime,
    last_sync: SystemTime,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            tcp_connections: 0,
            udp_packets_received: 0,
            http_requests: 0,
            https_requests: 0,
            active_nodes: 0,
            total_messages: 0,
            start_time: SystemTime::now(),
            last_sync: SystemTime::now(),
        }
    }
}

pub struct BellandeMeshSync {
    config: Arc<Config>,
    nodes: Arc<RwLock<Vec<Node>>>,
    running: Arc<RwLock<bool>>,
    tls_config: Arc<ServerConfig>,
    http_client: Client<hyper::client::HttpConnector>,
    https_client: Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>>,
    stats: Arc<RwLock<NetworkStats>>,
    metrics: Arc<MetricsManager>,
    message_tx: mpsc::Sender<(Message, SocketAddr)>,
    message_rx: Arc<Mutex<mpsc::Receiver<(Message, SocketAddr)>>>,
    cancel_token: CancellationToken,
}

#[async_trait::async_trait]
pub trait MeshTransport: Send + Sync {
    async fn start(&self) -> Result<(), BellandeMeshError>;
    async fn stop(&self) -> Result<(), BellandeMeshError>;
    async fn broadcast_data(&self, data: Vec<u8>) -> Result<(), BellandeMeshError>;
    async fn get_network_stats(&self) -> Result<NetworkStats, BellandeMeshError>;
    async fn get_all_nodes(&self) -> Result<Vec<Node>, BellandeMeshError>;
    async fn is_node_connected(&self, node_id: &NodeId) -> Result<bool, BellandeMeshError>;
    async fn send_data_to_node(
        &self,
        node_id: NodeId,
        data: Vec<u8>,
    ) -> Result<(), BellandeMeshError>;
    async fn restore_data_chunks(
        &self,
        node_id: NodeId,
        chunks: Vec<DataChunk>,
    ) -> Result<(), BellandeMeshError>;
    async fn start_metrics_collection(&self, interval: u64) -> Result<(), BellandeMeshError>;
    async fn set_max_connections(&self, max_conn: usize) -> Result<(), BellandeMeshError>;
}

impl BellandeMeshSync {
    pub fn new_with_metrics(
        config: &Config,
        metrics: Arc<MetricsManager>,
    ) -> Result<Self, BellandeMeshError> {
        let tls_config = Self::create_tls_config()?;
        let http_client = Client::new();
        let https_connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .build();
        let https_client = Client::builder().build(https_connector);

        let (message_tx, message_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let stats = Arc::new(RwLock::new(NetworkStats::default()));

        Ok(Self {
            config: Arc::new(config.clone()),
            nodes: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(true)),
            tls_config: Arc::new(tls_config),
            http_client,
            https_client,
            stats,
            metrics,
            message_tx,
            message_rx: Arc::new(Mutex::new(message_rx)),
            cancel_token: CancellationToken::new(),
        })
    }

    pub fn new_with_certs(
        config: &Config,
        cert_path: std::path::PathBuf,
        key_path: std::path::PathBuf,
        metrics: Arc<MetricsManager>,
    ) -> Result<Self, BellandeMeshError> {
        let tls_config = Self::create_tls_config_from_files(&cert_path, &key_path)?;
        let http_client = Client::new();
        let https_connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .build();
        let https_client = Client::builder().build(https_connector);

        let (message_tx, message_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let stats = Arc::new(RwLock::new(NetworkStats::default()));

        Ok(Self {
            config: Arc::new(config.clone()),
            nodes: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(true)),
            tls_config: Arc::new(tls_config),
            http_client,
            https_client,
            stats,
            metrics,
            message_tx,
            message_rx: Arc::new(Mutex::new(message_rx)),
            cancel_token: CancellationToken::new(),
        })
    }

    // Default TLS configuration using embedded certificates
    fn create_tls_config() -> Result<ServerConfig, BellandeMeshError> {
        let cert_path = Path::new("certs/server.crt");
        let key_path = Path::new("certs/server.key");
        Self::create_tls_config_from_files(cert_path, key_path)
    }

    fn create_tls_config_from_files(
        cert_path: &Path,
        key_path: &Path,
    ) -> Result<ServerConfig, BellandeMeshError> {
        let cert_data = fs::read(cert_path)
            .map_err(|e| BellandeMeshError::Custom(format!("Failed to read certificate: {}", e)))?;
        let key_data = fs::read(key_path)
            .map_err(|e| BellandeMeshError::Custom(format!("Failed to read key: {}", e)))?;

        let cert = Certificate(cert_data);
        let key = PrivateKey(key_data);

        let config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .map_err(|e| BellandeMeshError::Custom(format!("TLS config error: {}", e)))?;

        Ok(config)
    }

    pub async fn start(&self) -> Result<(), BellandeMeshError> {
        let mut running = self
            .running
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        *running = true;
        drop(running);

        self.start_message_handler().await?;
        self.start_protocol_listeners().await?;
        self.start_maintenance_tasks().await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<(), BellandeMeshError> {
        let mut running = self
            .running
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        *running = false;
        Ok(())
    }

    async fn start_message_handler(&self) -> Result<(), BellandeMeshError> {
        let handler = self.clone();
        tokio::spawn(async move {
            let mut rx = handler.message_rx.lock().await;
            while let Some((message, addr)) = rx.recv().await {
                if let Err(e) = handler.handle_message_internal(message, addr).await {
                    eprintln!("Message handling error from {}: {}", addr, e);
                }
            }
        });
        Ok(())
    }

    async fn start_protocol_listeners(&self) -> Result<(), BellandeMeshError> {
        self.start_tcp_listener().await?;
        self.start_udp_listener().await?;
        self.start_http_server().await?;
        self.start_https_server().await?;
        Ok(())
    }

    async fn start_tcp_listener(&self) -> Result<(), BellandeMeshError> {
        let addr = self
            .config
            .listen_address
            .parse::<SocketAddr>()
            .map_err(|e| BellandeMeshError::Custom(format!("Invalid address: {}", e)))?;
        let listener = TokioTcpListener::bind(addr).await?;
        let handler = self.clone();

        tokio::spawn(async move {
            while handler.is_running().unwrap_or(false) {
                if let Ok((stream, addr)) = listener.accept().await {
                    let handler = handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handler.handle_tcp_connection(stream).await {
                            eprintln!("TCP error from {}: {}", addr, e);
                        }
                    });
                }
            }
        });

        Ok(())
    }

    async fn start_udp_listener(&self) -> Result<(), BellandeMeshError> {
        let addr = self
            .config
            .listen_address
            .parse::<SocketAddr>()
            .map_err(|e| BellandeMeshError::Custom(format!("Invalid address: {}", e)))?;
        let socket = TokioUdpSocket::bind(addr).await?;
        let handler = self.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; UDP_BUFFER_SIZE];
            while handler.is_running().unwrap_or(false) {
                match socket.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        let handler = handler.clone();
                        let data = buf[..len].to_vec();
                        tokio::spawn(async move {
                            if let Err(e) = handler.handle_udp_packet(&data, src).await {
                                eprintln!("UDP error from {}: {}", src, e);
                            }
                        });
                    }
                    Err(e) => eprintln!("UDP receive error: {}", e),
                }
            }
        });

        Ok(())
    }

    async fn start_http_server(&self) -> Result<(), BellandeMeshError> {
        let addr = self
            .config
            .listen_address
            .parse::<SocketAddr>()
            .map_err(|e| BellandeMeshError::Custom(format!("Invalid address: {}", e)))?;
        let http_addr = SocketAddr::new(addr.ip(), addr.port() + HTTP_PORT_OFFSET);

        let metrics = self.metrics.clone();
        let handler = self.clone();

        let make_service = make_service_fn(move |_| {
            let handler = handler.clone();
            let metrics = metrics.clone();

            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let handler = handler.clone();
                    let metrics = metrics.clone();

                    async move {
                        metrics.increment_sync_operations();
                        handler.handle_http_request(req).await
                    }
                }))
            }
        });

        let server = Server::bind(&http_addr)
            .serve(make_service)
            .with_graceful_shutdown(Self::shutdown_signal());

        tokio::spawn(server);
        Ok(())
    }

    async fn shutdown_signal() {
        if let Err(e) = signal::ctrl_c().await {
            eprintln!("Failed to install CTRL+C signal handler: {}", e);
        }
    }

    async fn start_https_server(&self) -> Result<(), BellandeMeshError> {
        let addr = self
            .config
            .listen_address
            .parse::<SocketAddr>()
            .map_err(|e| BellandeMeshError::Custom(format!("Invalid address: {}", e)))?;
        let https_addr = SocketAddr::new(addr.ip(), addr.port() + HTTPS_PORT_OFFSET);
        let handler = self.clone();
        let tls_config = self.tls_config.clone();

        let listener = TokioTcpListener::bind(https_addr).await?;

        tokio::spawn(async move {
            while handler.is_running().unwrap_or(false) {
                if let Ok((stream, addr)) = listener.accept().await {
                    let handler = handler.clone();
                    let tls_config = tls_config.clone();

                    tokio::spawn(async move {
                        // Create TLS server connection
                        let tls_conn = match rustls::ServerConnection::new(tls_config.clone()) {
                            Ok(conn) => conn,
                            Err(e) => {
                                eprintln!("TLS connection creation error: {}", e);
                                return;
                            }
                        };

                        // Create async TLS stream
                        let mut tls_stream = tokio_rustls::server::TlsStream::new(stream, tls_conn);

                        // Perform TLS handshake
                        match tls_stream.do_handshake().await {
                            Ok(_) => {
                                // Handle secure connection
                                if let Err(e) = handler.handle_https_connection(tls_stream).await {
                                    eprintln!("HTTPS error from {}: {}", addr, e);
                                }
                            }
                            Err(e) => {
                                eprintln!("TLS handshake error from {}: {}", addr, e);
                            }
                        }
                    });
                }
            }
        });

        Ok(())
    }

    async fn start_maintenance_tasks(&self) -> Result<(), BellandeMeshError> {
        let sync_handler = self.clone();
        tokio::spawn(async move {
            while sync_handler.is_running().unwrap_or(false) {
                if let Err(e) = sync_handler.sync_with_peers().await {
                    eprintln!("Sync error: {}", e);
                }
                sleep(SYNC_INTERVAL).await;
            }
        });

        let cleanup_handler = self.clone();
        tokio::spawn(async move {
            while cleanup_handler.is_running().unwrap_or(false) {
                if let Err(e) = cleanup_handler.cleanup_dead_nodes().await {
                    eprintln!("Cleanup error: {}", e);
                }
                sleep(CLEANUP_INTERVAL).await;
            }
        });

        let ping_handler = self.clone();
        tokio::spawn(async move {
            while ping_handler.is_running().unwrap_or(false) {
                if let Err(e) = ping_handler.send_ping_to_all_nodes().await {
                    eprintln!("Ping error: {}", e);
                }
                sleep(PING_INTERVAL).await;
            }
        });

        Ok(())
    }

    async fn handle_tcp_connection<S>(&self, stream: S) -> Result<(), BellandeMeshError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (mut reader, mut writer) = tokio::io::split(stream);
        let mut buf = [0u8; MAX_MESSAGE_SIZE];

        while self.is_running()? {
            match self.read_async_message(&mut reader, &mut buf).await {
                Ok(message) => {
                    self.update_stats(|stats| stats.total_messages += 1);
                    if let Err(e) = self.write_async_message(&mut writer, &message).await {
                        eprintln!("Write error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Read error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn read_async_message<S>(
        &self,
        reader: &mut S,
        buf: &mut [u8],
    ) -> Result<Message, BellandeMeshError>
    where
        S: tokio::io::AsyncRead + Unpin,
    {
        use tokio::io::AsyncReadExt;

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(BellandeMeshError::Custom("Message too large".to_string()));
        }

        reader.read_exact(&mut buf[..len]).await?;
        bincode::deserialize(&buf[..len])
            .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))
    }

    async fn write_async_message<S>(
        &self,
        writer: &mut S,
        message: &Message,
    ) -> Result<(), BellandeMeshError>
    where
        S: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;

        let data = bincode::serialize(message)
            .map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;

        if data.len() > MAX_MESSAGE_SIZE {
            return Err(BellandeMeshError::Custom("Message too large".to_string()));
        }

        writer.write_all(&(data.len() as u32).to_be_bytes()).await?;
        writer.write_all(&data).await?;
        writer.flush().await?;

        Ok(())
    }

    async fn handle_udp_packet(
        &self,
        data: &[u8],
        src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let message = bincode::deserialize(data)
            .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))?;

        self.update_stats(|stats| stats.udp_packets_received += 1);
        self.handle_message_internal(message, src).await
    }

    async fn handle_http_request(&self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        self.update_stats(|stats| stats.http_requests += 1);

        let response = match (req.method(), req.uri().path()) {
            (&hyper::Method::GET, "/status") => {
                let status = self.get_status().await;
                Response::new(Body::from(status))
            }
            (&hyper::Method::POST, "/join") => match hyper::body::to_bytes(req.into_body()).await {
                Ok(bytes) => match self.handle_join_request(bytes.to_vec()).await {
                    Ok(_) => Response::new(Body::from("Joined successfully")),
                    Err(e) => Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(format!("Join failed: {}", e)))
                        .unwrap(),
                },
                Err(e) => Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("Invalid request: {}", e)))
                    .unwrap(),
            },
            _ => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap(),
        };

        Ok(response)
    }

    async fn handle_https_connection<S>(&self, stream: S) -> Result<(), BellandeMeshError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        self.update_stats(|stats| stats.https_requests += 1);
        self.handle_tcp_connection(stream).await
    }

    async fn handle_message_internal(
        &self,
        message: Message,
        src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        match message {
            Message::JoinRequest { id, public_key } => {
                self.handle_join_request_internal(id, public_key, src)
                    .await?;
            }
            Message::DataSync { chunks } => {
                self.handle_data_sync(chunks).await?;
            }
            Message::DataRequest { ids } => {
                self.handle_data_request(&ids, src).await?;
            }
            Message::Ping { sender: _, token } => {
                self.handle_ping(token, src).await?;
            }
            Message::Pong { sender, token: _ } => {
                self.handle_pong(sender, src).await?;
            }
            Message::Store {
                key,
                value,
                sender: _,
                token: _,
            } => {
                self.handle_store(key, value, src).await?;
            }
            Message::FindNode {
                target,
                sender: _,
                token: _,
            } => {
                self.handle_find_node(target, src).await?;
            }
            Message::FindValue {
                key,
                sender: _,
                token: _,
            } => {
                self.handle_find_value(key, src).await?;
            }
            Message::Nodes {
                nodes,
                sender: _,
                token: _,
            } => {
                self.handle_nodes(nodes, src).await?;
            }
            Message::Value {
                key,
                value,
                sender: _,
                token: _,
            } => {
                self.handle_value(key, value, src).await?;
            }
            Message::Heartbeat => {
                self.update_node_last_seen(src).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_store(
        &self,
        key: Vec<u8>,
        value: Vec<u8>,
        src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        self.metrics.increment_sync_operations();

        let chunk = DataChunk::new(
            value.clone(),
            "checksum".to_string(),
            0,
            self.get_local_id()?,
            vec![],
        );

        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;
        if let Some(node) = nodes.iter().find(|n| n.address == src) {
            let mut data = node
                .data
                .write()
                .map_err(|_| BellandeMeshError::LockError)?;
            data.insert(NodeId::from_bytes(&key), chunk);

            // Replicate to closest nodes
            let closest_nodes = self.find_closest_nodes(&NodeId::from_bytes(&key), 3);
            for target_node in closest_nodes {
                let store_msg = Message::Store {
                    key: key.clone(),
                    value: value.clone(),
                    sender: self.get_local_id()?,
                    token: rand::random(),
                };
                if let Err(e) = self.send_message(target_node.address, &store_msg).await {
                    eprintln!(
                        "Failed to replicate store to {}: {}",
                        target_node.address, e
                    );
                }
            }
        }

        Ok(())
    }

    async fn handle_find_node(
        &self,
        target: NodeId,
        src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let closest_nodes = self.find_closest_nodes(&target, 20).await;
        let response = Message::Nodes {
            nodes: closest_nodes,
            sender: self.get_local_id().await?,
            token: rand::random(),
        };
        self.send_message(src, &response).await?;
        Ok(())
    }

    async fn handle_find_value(
        &self,
        key: Vec<u8>,
        src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let node_id = NodeId::from_bytes(&key);
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;

        for node in nodes.iter() {
            if let Ok(data) = node.data.read() {
                if let Some(chunk) = data.get(&node_id) {
                    let response = Message::Value {
                        key: key.clone(),
                        value: chunk.content.clone(),
                        sender: self.get_local_id()?,
                        token: rand::random(),
                    };
                    return self.send_message(src, &response).await;
                }
            }
        }

        // If not found, return closest nodes
        let closest_nodes = self.find_closest_nodes(&node_id, 20);
        let response = Message::Nodes {
            nodes: closest_nodes,
            sender: self.get_local_id()?,
            token: rand::random(),
        };
        self.send_message(src, &response).await?;
        Ok(())
    }

    async fn handle_nodes(
        &self,
        new_nodes: Vec<Node>,
        _src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let mut nodes = self.nodes.write().await;
        for new_node in new_nodes {
            if !nodes.iter().any(|n| n.id == new_node.id) {
                nodes.push(new_node);
                self.metrics.increment_sync_operations();
            }
        }
        Ok(())
    }

    async fn handle_value(
        &self,
        key: Vec<u8>,
        value: Vec<u8>,
        _src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let chunk = DataChunk::new(
            value,
            "checksum".to_string(),
            0,
            self.get_local_id()?,
            vec![],
        );

        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        if let Some(node) = nodes.first_mut() {
            if let Ok(mut data) = node.data.write() {
                data.insert(NodeId::from_bytes(&key), chunk);
                self.metrics.increment_sync_operations();
            }
        }
        Ok(())
    }

    async fn find_closest_nodes(&self, target: &NodeId, count: usize) -> Vec<Node> {
        let nodes = self.nodes.read().await;
        let mut distances: Vec<_> = nodes
            .iter()
            .map(|node| (node.clone(), self.calculate_distance(&node.id, target)))
            .collect();

        distances.sort_by(|a, b| a.1.cmp(&b.1));
        distances
            .into_iter()
            .take(count)
            .map(|(node, _)| node)
            .collect()
    }

    // XOR distance metric for Kademlia-like routing
    fn calculate_distance(&self, node1: &NodeId, node2: &NodeId) -> u64 {
        let n1_bytes = node1.to_bytes();
        let n2_bytes = node2.to_bytes();
        let mut distance = 0u64;

        for i in 0..8 {
            if i < n1_bytes.len() && i < n2_bytes.len() {
                distance ^= (n1_bytes[i] ^ n2_bytes[i]) as u64;
            }
        }

        distance
    }

    async fn update_node_last_seen(&self, addr: SocketAddr) -> Result<(), BellandeMeshError> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        if let Some(node) = nodes.iter_mut().find(|n| n.address == addr) {
            node.update_last_seen();
        }
        Ok(())
    }

    async fn handle_join_request(&self, data: Vec<u8>) -> Result<(), BellandeMeshError> {
        let request: Message = bincode::deserialize(&data)
            .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))?;

        match request {
            Message::JoinRequest { id, public_key } => {
                let addr = SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                    rand::random::<u16>(),
                );
                self.handle_join_request_internal(id, public_key, addr)
                    .await
            }
            _ => Err(BellandeMeshError::Custom(
                "Invalid message type".to_string(),
            )),
        }
    }

    async fn handle_join_request_internal(
        &self,
        id: NodeId,
        public_key: PublicKey,
        addr: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let new_node = Node::new(id, addr, public_key);

        {
            let mut nodes = self
                .nodes
                .write()
                .map_err(|_| BellandeMeshError::LockError)?;
            if !nodes.iter().any(|n| n.id == new_node.id) {
                nodes.push(new_node.clone());
                self.update_stats(|stats| stats.active_nodes += 1);
            }
        }

        self.broadcast_new_node(&new_node).await?;
        Ok(())
    }

    async fn handle_data_sync(&self, chunks: Vec<DataChunk>) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;

        for chunk in chunks {
            if let Some(node) = nodes.iter().find(|n| n.id == chunk.author) {
                let _ = node.add_data_chunk(chunk);
            }
        }

        Ok(())
    }

    async fn handle_data_request(
        &self,
        ids: &[NodeId],
        src: SocketAddr,
    ) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;
        let mut chunks = Vec::new();

        for node in nodes.iter() {
            for id in ids {
                if let Some(chunk) = node.get_data_chunk(id) {
                    chunks.push(chunk);
                }
            }
        }

        let response = Message::DataSync { chunks };
        self.send_message(src, &response).await?;

        Ok(())
    }

    async fn handle_ping(&self, token: u64, src: SocketAddr) -> Result<(), BellandeMeshError> {
        let response = Message::Pong {
            sender: self.get_local_id()?,
            token,
        };
        self.send_message(src, &response).await?;
        Ok(())
    }

    async fn handle_pong(&self, sender: NodeId, src: SocketAddr) -> Result<(), BellandeMeshError> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;
        if let Some(node) = nodes.iter_mut().find(|n| n.id == sender) {
            node.update_last_seen();
        }
        Ok(())
    }

    async fn restore_nodes(&self, nodes: Vec<Node>) -> Result<(), BellandeMeshError> {
        let mut current_nodes = self.nodes.write().await;
        for node in nodes {
            if !current_nodes.iter().any(|n| n.id == node.id) {
                self.metrics.increment_sync_operations();
                current_nodes.push(node);
            }
        }
        Ok(())
    }

    pub async fn send_message(
        &self,
        addr: SocketAddr,
        msg: &Message,
    ) -> Result<(), BellandeMeshError> {
        let data =
            bincode::serialize(msg).map_err(|e| BellandeMeshError::Serialization(e.to_string()))?;

        if data.len() > MAX_MESSAGE_SIZE {
            return Err(BellandeMeshError::Custom("Message too large".to_string()));
        }

        let mut stream =
            TcpStream::connect(addr).map_err(|e| BellandeMeshError::NetworkError(e.to_string()))?;

        stream.write_all(&(data.len() as u32).to_be_bytes())?;
        stream.write_all(&data)?;
        stream.flush()?;

        Ok(())
    }

    async fn sync_with_peers(&self) -> Result<(), BellandeMeshError> {
        let nodes = {
            let nodes_guard = self
                .nodes
                .read()
                .map_err(|_| BellandeMeshError::LockError)?;
            nodes_guard.to_vec()
        };

        for node in nodes {
            if let Ok(mut stream) = TcpStream::connect(node.address) {
                let chunks = {
                    let data = node.data.read().map_err(|_| BellandeMeshError::LockError)?;
                    data.keys().cloned().collect::<Vec<_>>()
                };

                let request = Message::DataRequest { ids: chunks };
                if let Err(e) = self.send_message(node.address, &request).await {
                    eprintln!("Failed to sync with {}: {}", node.address, e);
                    continue;
                }

                match self.read_message(&mut stream).await {
                    Ok(Message::DataSync { chunks }) => {
                        self.handle_data_sync(chunks).await?;
                    }
                    Ok(_) => eprintln!("Unexpected response from {}", node.address),
                    Err(e) => eprintln!("Error reading sync response from {}: {}", node.address, e),
                }
            }
        }

        self.update_stats(|stats| stats.last_sync = SystemTime::now());
        Ok(())
    }

    pub async fn restore_data_chunks(
        &self,
        node_id: NodeId,
        chunks: Vec<DataChunk>,
    ) -> Result<(), BellandeMeshError> {
        let nodes = self.nodes.read().await;
        if let Some(node) = nodes.iter().find(|n| n.id == node_id) {
            let mut data = node
                .data
                .write()
                .map_err(|_| BellandeMeshError::LockError)?;
            for chunk in chunks {
                data.insert(chunk.id.clone(), chunk);
                self.metrics.increment_sync_operations();
            }
        }
        Ok(())
    }

    async fn send_response(
        &self,
        src: SocketAddr,
        closest_nodes: Vec<Node>,
    ) -> Result<(), BellandeMeshError> {
        let response = Message::Nodes {
            nodes: closest_nodes,
            sender: self.get_local_id().await?,
            token: rand::random(),
        };
        self.send_message(src, &response).await
    }

    async fn start_metrics_collection(&self) -> Result<(), BellandeMeshError> {
        let metrics = self.metrics.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            while *running.read().await {
                interval.tick().await;
                if let Err(e) = metrics.update_metrics().await {
                    eprintln!("Metrics error: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn set_max_connections(&self, max_conn: usize) -> Result<(), BellandeMeshError> {
        let nodes = self.nodes.read().await;
        if nodes.len() > max_conn {
            // Remove excess connections, keeping the oldest/most reliable ones
            let mut nodes = self.nodes.write().await;
            nodes.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
            nodes.truncate(max_conn);
            self.update_stats(|stats| stats.active_nodes = max_conn)
                .await;
        }
        Ok(())
    }

    pub async fn broadcast_data(&self, data: Vec<u8>) -> Result<(), BellandeMeshError> {
        let nodes = self.nodes.read().await;
        let message = Message::Store {
            key: vec![], // Broadcast key
            value: data,
            sender: self.get_local_id().await?,
            token: rand::random(),
        };

        let mut send_errors = Vec::new();
        for node in nodes.iter() {
            if let Err(e) = self.send_message(node.address, &message).await {
                send_errors.push((node.id.clone(), e));
            }
        }

        // Update metrics
        self.metrics.increment_sync_operations();
        self.update_stats(|stats| stats.total_messages += 1).await;

        // If any errors occurred, return the first one
        if let Some((node_id, error)) = send_errors.first() {
            return Err(BellandeMeshError::Custom(format!(
                "Failed to broadcast to node {}: {}",
                node_id, error
            )));
        }

        Ok(())
    }

    pub async fn get_network_stats(&self) -> Result<NetworkStats, BellandeMeshError> {
        let stats = self.stats.read().await;
        Ok(NetworkStats {
            tcp_connections: stats.tcp_connections,
            udp_packets_received: stats.udp_packets_received,
            http_requests: stats.http_requests,
            https_requests: stats.https_requests,
            active_nodes: self.nodes.read().await.len(),
            total_messages: stats.total_messages,
            start_time: stats.start_time,
            last_sync: stats.last_sync,
        })
    }

    pub async fn send_data_to_node(
        &self,
        node_id: NodeId,
        data: Vec<u8>,
    ) -> Result<(), BellandeMeshError> {
        let nodes = self.nodes.read().await;
        let node = nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or_else(|| BellandeMeshError::Custom("Node not found".to_string()))?;

        let message = Message::Store {
            key: node_id.to_bytes(),
            value: data,
            sender: self.get_local_id().await?,
            token: rand::random(),
        };

        // Send the message
        self.send_message(node.address, &message).await?;

        // Update metrics
        self.metrics.increment_sync_operations();
        self.update_stats(|stats| stats.total_messages += 1).await;

        Ok(())
    }

    async fn cleanup_dead_nodes(&self) -> Result<(), BellandeMeshError> {
        let timeout = Duration::from_secs(self.config.node_timeout);
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| BellandeMeshError::LockError)?;

        let initial_count = nodes.len();
        nodes.retain(|node| {
            let is_alive = node.is_alive(timeout);
            if !is_alive {
                eprintln!("Removing dead node: {}", node.address);
            }
            is_alive
        });

        let removed = initial_count - nodes.len();
        if removed > 0 {
            self.update_stats(|stats| stats.active_nodes -= removed);
        }

        Ok(())
    }

    async fn send_ping_to_all_nodes(&self) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;
        let token = rand::random::<u64>();
        let ping = Message::Ping {
            sender: self.get_local_id()?,
            token,
        };

        for node in nodes.iter() {
            if let Err(e) = self.send_message(node.address, &ping).await {
                eprintln!("Failed to ping {}: {}", node.address, e);
            }
        }

        Ok(())
    }

    async fn read_message(&self, stream: &mut TcpStream) -> Result<Message, BellandeMeshError> {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(BellandeMeshError::Custom("Message too large".to_string()));
        }

        let mut msg_buf = vec![0u8; len];
        stream.read_exact(&mut msg_buf)?;

        bincode::deserialize(&msg_buf)
            .map_err(|e| BellandeMeshError::Deserialization(e.to_string()))
    }

    async fn get_status(&self) -> String {
        let stats = self
            .stats
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| NetworkStats::default());

        serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "Error getting status".to_string())
    }

    fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut NetworkStats),
    {
        if let Ok(mut stats) = self.stats.write() {
            updater(&mut stats);
        }
    }

    fn is_running(&self) -> Result<bool, BellandeMeshError> {
        self.running
            .read()
            .map_err(|_| BellandeMeshError::LockError)
            .map(|guard| *guard)
    }

    pub async fn get_local_id(&self) -> Result<NodeId, BellandeMeshError> {
        self.nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)
            .map(|nodes| nodes.first().map(|n| n.id).unwrap_or_else(|| NodeId::new()))
    }

    async fn broadcast_new_node(&self, new_node: &Node) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;
        let message = Message::JoinResponse {
            accepted: true,
            nodes: vec![new_node.clone()],
        };

        for node in nodes.iter() {
            if node.id != new_node.id {
                if let Err(e) = self.send_message(node.address, &message).await {
                    eprintln!("Failed to broadcast new node to {}: {}", node.address, e);
                }
            }
        }

        Ok(())
    }

    pub async fn broadcast_message(&self, message: Message) -> Result<(), BellandeMeshError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?;

        for node in nodes.iter() {
            if let Err(e) = self.send_message(node.address, &message).await {
                eprintln!("Failed to broadcast message to {}: {}", node.address, e);
            }
        }

        Ok(())
    }

    pub async fn get_node_count(&self) -> Result<usize, BellandeMeshError> {
        Ok(self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?
            .len())
    }

    pub async fn get_node_list(&self) -> Result<Vec<NodeId>, BellandeMeshError> {
        Ok(self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?
            .iter()
            .map(|node| node.id)
            .collect())
    }

    pub async fn is_node_connected(&self, node_id: &NodeId) -> Result<bool, BellandeMeshError> {
        Ok(self
            .nodes
            .read()
            .map_err(|_| BellandeMeshError::LockError)?
            .iter()
            .any(|node| &node.id == node_id))
    }
}

impl Clone for BellandeMeshSync {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            nodes: Arc::clone(&self.nodes),
            running: Arc::clone(&self.running),
            tls_config: Arc::clone(&self.tls_config),
            http_client: self.http_client.clone(),
            https_client: self.https_client.clone(),
            stats: Arc::clone(&self.stats),
            metrics: Arc::clone(&self.metrics),
            message_tx: self.message_tx.clone(),
            message_rx: Arc::clone(&self.message_rx),
            cancel_token: CancellationToken::new(),
        }
    }
}
