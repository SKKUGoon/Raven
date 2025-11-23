// gRPC Server Modules
//
// Structured into separate services:
// - client_service: Market Data API for trading clients
// - controller_service: Control API for ravenctl admin tool

pub mod client_service;
pub mod controller_service;

pub use client_service::MarketDataServer;
