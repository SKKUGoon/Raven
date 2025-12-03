pub mod proto {
    tonic::include_proto!("raven");
}

pub mod config;
pub mod db;
pub mod source;
pub mod features;
pub mod service;
pub mod telemetry;
// pub mod config; // To be implemented
// pub mod utils; // To be implemented
