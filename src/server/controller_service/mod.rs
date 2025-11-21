// Control module - Dynamic collector management
// "The Hand of the King - orchestrating the realm's data collection"

pub mod grpc_service;
pub mod manager;

pub use grpc_service::ControlServiceImpl;
pub use manager::CollectorManager;
