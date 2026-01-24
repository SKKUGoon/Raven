pub mod proto {
    tonic::include_proto!("raven");
}

pub mod config;
pub mod db;
pub mod domain;
pub mod features;
pub mod pipeline;
pub mod routing;
pub mod service;
pub mod source;
pub mod telemetry;
pub mod utils;
