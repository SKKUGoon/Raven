pub mod manager;
pub mod grpc_service;

pub use grpc_service::ControlServiceImpl;
pub use manager::CollectorManager;
