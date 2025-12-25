pub mod proto {
    tonic::include_proto!("raven");
}

pub mod config;
pub mod domain;
pub mod routing;
pub mod db;
pub mod source;
pub mod features;
pub mod service;
pub mod telemetry;
pub mod utils;
pub mod pipeline;
// pub mod config; // To be implemented
// pub mod utils; // To be implemented
