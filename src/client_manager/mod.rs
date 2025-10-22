// Client Connection Manager
// "Managing the ravens that carry our messages"

use crate::error::RavenResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

/// Client connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ClientState {
    /// Client is connected and active
    Connected,
    /// Client is disconnecting gracefully
    Disconnecting,
    /// Client has disconnected
    Disconnected,
    /// Client connection is unhealthy (missed heartbeats)
    Unhealthy,
}

impl std::fmt::Display for ClientState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "connected"),
            Self::Disconnecting => write!(f, "disconnecting"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Client connection information
#[derive(Debug, Clone)]
pub struct ClientConnection {
    /// Unique client identifier
    pub client_id: String,
    /// Client connection state
    pub state: ClientState,
    /// When the client connected
    pub connected_at: Instant,
    /// Last heartbeat received
    pub last_heartbeat: Instant,
    /// Last activity (message sent/received)
    pub last_activity: Instant,
    /// Client metadata (IP, user agent, etc.)
    pub metadata: HashMap<String, String>,
    /// Number of active subscriptions
    pub subscription_count: usize,
    /// Total messages sent to this client
    pub messages_sent: u64,
    /// Total messages received from this client
    pub messages_received: u64,
    /// Connection quality metrics
    pub connection_quality: ConnectionQuality,
}

impl ClientConnection {
    /// Create a new client connection
    pub fn new<S: Into<String>>(client_id: S) -> Self {
        let now = Instant::now();
        Self {
            client_id: client_id.into(),
            state: ClientState::Connected,
            connected_at: now,
            last_heartbeat: now,
            last_activity: now,
            metadata: HashMap::new(),
            subscription_count: 0,
            messages_sent: 0,
            messages_received: 0,
            connection_quality: ConnectionQuality::new(),
        }
    }

    /// Update last heartbeat time
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
        self.last_activity = self.last_heartbeat;
        self.connection_quality.record_heartbeat();
    }

    /// Update last activity time
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Increment messages sent counter
    pub fn increment_messages_sent(&mut self) {
        self.messages_sent += 1;
        self.update_activity();
    }

    /// Increment messages received counter
    pub fn increment_messages_received(&mut self) {
        self.messages_received += 1;
        self.update_activity();
    }

    /// Check if client is healthy based on heartbeat timeout
    pub fn is_healthy(&self, heartbeat_timeout: Duration) -> bool {
        match self.state {
            ClientState::Connected => self.last_heartbeat.elapsed() <= heartbeat_timeout,
            ClientState::Disconnecting | ClientState::Disconnected | ClientState::Unhealthy => {
                false
            }
        }
    }

    /// Get connection duration
    pub fn connection_duration(&self) -> Duration {
        self.connected_at.elapsed()
    }

    /// Get time since last activity
    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Add metadata
    pub fn add_metadata<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.metadata.insert(key.into(), value.into());
    }
}

/// Connection quality metrics
#[derive(Debug, Clone)]
pub struct ConnectionQuality {
    /// Total heartbeats received
    pub heartbeats_received: u64,
    /// Missed heartbeats
    pub missed_heartbeats: u64,
    /// Average heartbeat interval
    pub avg_heartbeat_interval: Duration,
    /// Connection stability score (0.0 to 1.0)
    pub stability_score: f64,
    /// Last heartbeat intervals for calculating average
    heartbeat_intervals: Vec<Duration>,
    last_heartbeat_time: Option<Instant>,
}

impl ConnectionQuality {
    /// Create new connection quality tracker
    pub fn new() -> Self {
        Self {
            heartbeats_received: 0,
            missed_heartbeats: 0,
            avg_heartbeat_interval: Duration::from_secs(30),
            stability_score: 1.0,
            heartbeat_intervals: Vec::new(),
            last_heartbeat_time: None,
        }
    }

    /// Record a heartbeat
    pub fn record_heartbeat(&mut self) {
        let now = Instant::now();

        if let Some(last_time) = self.last_heartbeat_time {
            let interval = now.duration_since(last_time);
            self.heartbeat_intervals.push(interval);

            // Keep only last 10 intervals for average calculation
            if self.heartbeat_intervals.len() > 10 {
                self.heartbeat_intervals.remove(0);
            }

            // Calculate average interval
            let total: Duration = self.heartbeat_intervals.iter().sum();
            self.avg_heartbeat_interval = total / self.heartbeat_intervals.len() as u32;
        }

        self.heartbeats_received += 1;
        self.last_heartbeat_time = Some(now);
        self.update_stability_score();
    }

    /// Record a missed heartbeat
    pub fn record_missed_heartbeat(&mut self) {
        self.missed_heartbeats += 1;
        self.update_stability_score();
    }

    /// Update stability score based on heartbeat reliability
    fn update_stability_score(&mut self) {
        let total_heartbeats = self.heartbeats_received + self.missed_heartbeats;
        if total_heartbeats > 0 {
            self.stability_score = self.heartbeats_received as f64 / total_heartbeats as f64;
        }
    }
}

impl Default for ConnectionQuality {
    fn default() -> Self {
        Self::new()
    }
}

/// Client disconnection reason
#[derive(Debug, Clone)]
pub enum DisconnectionReason {
    /// Client initiated graceful disconnect
    ClientInitiated,
    /// Server initiated disconnect (e.g., shutdown)
    ServerInitiated,
    /// Connection timeout (missed heartbeats)
    Timeout,
    /// Connection error
    ConnectionError(String),
    /// Maximum connections exceeded
    MaxConnectionsExceeded,
    /// Authentication/authorization failure
    AuthenticationFailure,
    /// Protocol violation
    ProtocolViolation(String),
}

impl std::fmt::Display for DisconnectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientInitiated => write!(f, "client initiated"),
            Self::ServerInitiated => write!(f, "server initiated"),
            Self::Timeout => write!(f, "timeout"),
            Self::ConnectionError(msg) => write!(f, "connection error: {msg}"),
            Self::MaxConnectionsExceeded => write!(f, "max connections exceeded"),
            Self::AuthenticationFailure => write!(f, "authentication failure"),
            Self::ProtocolViolation(msg) => write!(f, "protocol violation: {msg}"),
        }
    }
}

/// Client disconnection event
#[derive(Debug, Clone)]
pub struct DisconnectionEvent {
    pub client_id: String,
    pub reason: DisconnectionReason,
    pub timestamp: Instant,
    pub connection_duration: Duration,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub subscription_count: usize,
}

/// Configuration for client manager
#[derive(Debug, Clone)]
pub struct ClientManagerConfig {
    /// Maximum number of concurrent clients
    pub max_clients: usize,
    /// Heartbeat timeout duration
    pub heartbeat_timeout: Duration,
    /// Interval for checking client health
    pub health_check_interval: Duration,
    /// Grace period for client disconnection
    pub disconnection_grace_period: Duration,
    /// Maximum idle time before disconnecting client
    pub max_idle_time: Duration,
    /// Whether to enable connection quality tracking
    pub track_connection_quality: bool,
}

impl Default for ClientManagerConfig {
    fn default() -> Self {
        Self {
            max_clients: 1000,
            heartbeat_timeout: Duration::from_secs(60),
            health_check_interval: Duration::from_secs(30),
            disconnection_grace_period: Duration::from_secs(10),
            max_idle_time: Duration::from_secs(300), // 5 minutes
            track_connection_quality: true,
        }
    }
}

/// Client manager for handling connections and disconnections
pub struct ClientManager {
    config: ClientManagerConfig,
    clients: Arc<RwLock<HashMap<String, ClientConnection>>>,
    disconnection_events: Arc<RwLock<Vec<DisconnectionEvent>>>,
    health_check_running: Arc<std::sync::atomic::AtomicBool>,
    disconnection_sender: mpsc::UnboundedSender<DisconnectionEvent>,
    disconnection_receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<DisconnectionEvent>>>,
}
impl ClientManager {
    /// Create a new client manager
    pub fn new(config: ClientManagerConfig) -> Self {
        info!(
            "⚮ Initializing client manager with max clients: {}",
            config.max_clients
        );

        let (disconnection_sender, disconnection_receiver) = mpsc::unbounded_channel();

        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            disconnection_events: Arc::new(RwLock::new(Vec::new())),
            health_check_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            disconnection_sender,
            disconnection_receiver: Arc::new(tokio::sync::Mutex::new(disconnection_receiver)),
        }
    }

    /// Register a new client connection
    pub async fn register_client(&self, client_id: String) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        // Check if we've reached the maximum number of clients
        if clients.len() >= self.config.max_clients {
            crate::raven_bail!(crate::raven_error!(
                max_connections_exceeded,
                clients.len(),
                self.config.max_clients,
            ));
        }

        // Check if client already exists
        if clients.contains_key(&client_id) {
            warn!(
                "⚮ Client {} already registered, updating connection",
                client_id
            );
        }

        let connection = ClientConnection::new(client_id.clone());
        clients.insert(client_id.clone(), connection);

        info!(
            "⚮ Registered client: {} (total: {})",
            client_id,
            clients.len()
        );
        Ok(())
    }

    /// Update client heartbeat
    pub async fn update_heartbeat(&self, client_id: &str) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            client.update_heartbeat();
            debug!("♡ Updated heartbeat for client: {}", client_id);
            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Update client activity
    pub async fn update_activity(&self, client_id: &str) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            client.update_activity();
            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Increment messages sent to client
    pub async fn increment_messages_sent(&self, client_id: &str) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            client.increment_messages_sent();
            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Increment messages received from client
    pub async fn increment_messages_received(&self, client_id: &str) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            client.increment_messages_received();
            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Update client subscription count
    pub async fn update_subscription_count(
        &self,
        client_id: &str,
        count: usize,
    ) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            client.subscription_count = count;
            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Add metadata to client
    pub async fn add_client_metadata<K: Into<String>, V: Into<String>>(
        &self,
        client_id: &str,
        key: K,
        value: V,
    ) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            client.add_metadata(key, value);
            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Initiate graceful client disconnection
    pub async fn disconnect_client(
        &self,
        client_id: &str,
        reason: DisconnectionReason,
    ) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(client) = clients.get_mut(client_id) {
            if client.state == ClientState::Connected {
                client.state = ClientState::Disconnecting;

                info!(
                    "⚬ Initiating graceful disconnection for client: {} (reason: {})",
                    client_id, reason
                );

                // Create disconnection event
                let event = DisconnectionEvent {
                    client_id: client_id.to_string(),
                    reason,
                    timestamp: Instant::now(),
                    connection_duration: client.connection_duration(),
                    messages_sent: client.messages_sent,
                    messages_received: client.messages_received,
                    subscription_count: client.subscription_count,
                };

                // Send disconnection event
                if let Err(e) = self.disconnection_sender.send(event) {
                    error!("Failed to send disconnection event: {}", e);
                }

                // Schedule final cleanup after grace period
                let client_manager = self.clone();
                let client_id_clone = client_id.to_string();
                tokio::spawn(async move {
                    sleep(client_manager.config.disconnection_grace_period).await;
                    if let Err(e) = client_manager
                        .finalize_disconnection(&client_id_clone)
                        .await
                    {
                        error!(
                            "Failed to finalize disconnection for {}: {}",
                            client_id_clone, e
                        );
                    }
                });
            }

            Ok(())
        } else {
            crate::raven_bail!(crate::raven_error!(client_not_found, client_id));
        }
    }

    /// Finalize client disconnection (remove from registry)
    async fn finalize_disconnection(&self, client_id: &str) -> RavenResult<()> {
        let mut clients = self.clients.write().await;

        if let Some(mut client) = clients.remove(client_id) {
            client.state = ClientState::Disconnected;
            info!(
                "⚬ Finalized disconnection for client: {} (total: {})",
                client_id,
                clients.len()
            );
        }

        Ok(())
    }

    /// Get client connection information
    pub async fn get_client(&self, client_id: &str) -> Option<ClientConnection> {
        let clients = self.clients.read().await;
        clients.get(client_id).cloned()
    }

    /// Get all connected clients
    pub async fn get_all_clients(&self) -> Vec<ClientConnection> {
        let clients = self.clients.read().await;
        clients.values().cloned().collect()
    }

    /// Get client count
    pub async fn get_client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

    /// Get healthy client count
    pub async fn get_healthy_client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients
            .values()
            .filter(|c| c.is_healthy(self.config.heartbeat_timeout))
            .count()
    }

    /// Start health monitoring
    pub async fn start_health_monitoring(&self) -> RavenResult<()> {
        if self
            .health_check_running
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            let client_manager = self.clone();
            tokio::spawn(async move {
                client_manager.health_monitoring_loop().await;
            });

            info!("⚕ Started client health monitoring");
        }

        Ok(())
    }

    /// Stop health monitoring
    pub fn stop_health_monitoring(&self) {
        self.health_check_running
            .store(false, std::sync::atomic::Ordering::Relaxed);
        info!("⚕ Stopped client health monitoring");
    }

    /// Health monitoring loop
    async fn health_monitoring_loop(&self) {
        let mut interval = interval(self.config.health_check_interval);

        while self
            .health_check_running
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            interval.tick().await;

            if let Err(e) = self.check_client_health().await {
                error!("Health check failed: {}", e);
            }
        }
    }

    /// Check health of all clients
    async fn check_client_health(&self) -> RavenResult<()> {
        let mut unhealthy_clients = Vec::new();
        let mut idle_clients = Vec::new();

        {
            let mut clients = self.clients.write().await;

            for (client_id, client) in clients.iter_mut() {
                // Check heartbeat timeout
                if !client.is_healthy(self.config.heartbeat_timeout)
                    && client.state == ClientState::Connected
                {
                    client.state = ClientState::Unhealthy;
                    client.connection_quality.record_missed_heartbeat();
                    unhealthy_clients.push(client_id.clone());
                }

                // Check idle timeout
                if client.idle_duration() > self.config.max_idle_time
                    && client.state == ClientState::Connected
                {
                    idle_clients.push(client_id.clone());
                }
            }
        }

        // Disconnect unhealthy clients
        for client_id in unhealthy_clients {
            warn!("⚕ Disconnecting unhealthy client: {}", client_id);
            if let Err(e) = self
                .disconnect_client(&client_id, DisconnectionReason::Timeout)
                .await
            {
                error!("Failed to disconnect unhealthy client {}: {}", client_id, e);
            }
        }

        // Disconnect idle clients
        for client_id in idle_clients {
            info!("⚬ Disconnecting idle client: {}", client_id);
            if let Err(e) = self
                .disconnect_client(&client_id, DisconnectionReason::Timeout)
                .await
            {
                error!("Failed to disconnect idle client {}: {}", client_id, e);
            }
        }

        Ok(())
    }

    /// Start disconnection event processing
    pub async fn start_disconnection_processing(&self) -> RavenResult<()> {
        let client_manager = self.clone();
        tokio::spawn(async move {
            client_manager.disconnection_processing_loop().await;
        });

        info!("⚬ Started disconnection event processing");
        Ok(())
    }

    /// Disconnection event processing loop
    async fn disconnection_processing_loop(&self) {
        let mut receiver = self.disconnection_receiver.lock().await;

        while let Some(event) = receiver.recv().await {
            self.handle_disconnection_event(event).await;
        }
    }

    /// Handle a disconnection event
    async fn handle_disconnection_event(&self, event: DisconnectionEvent) {
        // Store the event for analytics
        {
            let mut events = self.disconnection_events.write().await;
            events.push(event.clone());

            // Keep only last 1000 events
            if events.len() > 1000 {
                events.remove(0);
            }
        }

        info!(
            "Client disconnection: {} (reason: {}, duration: {:?}, messages: sent={}, received={})",
            event.client_id,
            event.reason,
            event.connection_duration,
            event.messages_sent,
            event.messages_received
        );
    }

    /// Get disconnection events
    pub async fn get_disconnection_events(&self) -> Vec<DisconnectionEvent> {
        let events = self.disconnection_events.read().await;
        events.clone()
    }

    /// Get client statistics
    pub async fn get_client_stats(&self) -> HashMap<String, u64> {
        let clients = self.clients.read().await;
        let events = self.disconnection_events.read().await;

        let total_clients = clients.len() as u64;
        let healthy_clients = clients
            .values()
            .filter(|c| c.is_healthy(self.config.heartbeat_timeout))
            .count() as u64;
        let unhealthy_clients = total_clients - healthy_clients;

        let total_messages_sent: u64 = clients.values().map(|c| c.messages_sent).sum();
        let total_messages_received: u64 = clients.values().map(|c| c.messages_received).sum();
        let total_subscriptions: u64 = clients.values().map(|c| c.subscription_count as u64).sum();

        let total_disconnections = events.len() as u64;

        let mut stats = HashMap::new();
        stats.insert("total_clients".to_string(), total_clients);
        stats.insert("healthy_clients".to_string(), healthy_clients);
        stats.insert("unhealthy_clients".to_string(), unhealthy_clients);
        stats.insert("total_messages_sent".to_string(), total_messages_sent);
        stats.insert(
            "total_messages_received".to_string(),
            total_messages_received,
        );
        stats.insert("total_subscriptions".to_string(), total_subscriptions);
        stats.insert("total_disconnections".to_string(), total_disconnections);
        stats.insert("max_clients".to_string(), self.config.max_clients as u64);

        stats
    }

    /// Shutdown all clients gracefully
    pub async fn shutdown_all_clients(&self) -> RavenResult<()> {
        info!("■ Initiating graceful shutdown of all clients");

        let client_ids: Vec<String> = {
            let clients = self.clients.read().await;
            clients.keys().cloned().collect()
        };

        for client_id in client_ids {
            if let Err(e) = self
                .disconnect_client(&client_id, DisconnectionReason::ServerInitiated)
                .await
            {
                error!(
                    "Failed to disconnect client {} during shutdown: {}",
                    client_id, e
                );
            }
        }

        // Wait for grace period
        sleep(self.config.disconnection_grace_period).await;

        // Force disconnect any remaining clients
        let remaining_count = {
            let mut clients = self.clients.write().await;
            let count = clients.len();
            clients.clear();
            count
        };

        if remaining_count > 0 {
            warn!("■ Force disconnected {} remaining clients", remaining_count);
        }

        self.stop_health_monitoring();

        info!("■ All clients disconnected");
        Ok(())
    }
}

impl Clone for ClientManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            clients: Arc::clone(&self.clients),
            disconnection_events: Arc::clone(&self.disconnection_events),
            health_check_running: Arc::clone(&self.health_check_running),
            disconnection_sender: self.disconnection_sender.clone(),
            disconnection_receiver: Arc::clone(&self.disconnection_receiver),
        }
    }
}
