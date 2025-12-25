use std::collections::{HashMap, HashSet, VecDeque};

use crate::config::Settings;
use crate::domain::venue::VenueId;
use crate::proto::DataType;
use crate::utils::service_registry::{self, ServiceSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StreamKind {
    Trade,
    Orderbook,
    Candle,
}

impl StreamKind {
    pub fn as_proto_i32(self) -> i32 {
        match self {
            StreamKind::Trade => DataType::Trade as i32,
            StreamKind::Orderbook => DataType::Orderbook as i32,
            StreamKind::Candle => DataType::Candle as i32,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            StreamKind::Trade => "TRADE",
            StreamKind::Orderbook => "ORDERBOOK",
            StreamKind::Candle => "CANDLE",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeId {
    CollectorBinanceSpot,
    CollectorBinanceFutures,
    TickPersistence,
    BarPersistence,
    Timebar1m,
    Timebar1s,
    TibsSmall,
    TibsLarge,
    TrbsSmall,
    TrbsLarge,
    VibsSmall,
    VibsLarge,
    Vpin,
}

impl NodeId {
    pub fn label(self) -> &'static str {
        match self {
            NodeId::CollectorBinanceSpot => "collector:binance_spot",
            NodeId::CollectorBinanceFutures => "collector:binance_futures",
            NodeId::TickPersistence => "tick_persistence",
            NodeId::BarPersistence => "bar_persistence",
            NodeId::Timebar1m => "timebar_60s",
            NodeId::Timebar1s => "timebar_1s",
            NodeId::TibsSmall => "tibs_small",
            NodeId::TibsLarge => "tibs_large",
            NodeId::TrbsSmall => "trbs_small",
            NodeId::TrbsLarge => "trbs_large",
            NodeId::VibsSmall => "vibs_small",
            NodeId::VibsLarge => "vibs_large",
            NodeId::Vpin => "vpin",
        }
    }

    pub fn service_id(self) -> &'static str {
        match self {
            NodeId::CollectorBinanceSpot => "binance_spot",
            NodeId::CollectorBinanceFutures => "binance_futures",
            NodeId::TickPersistence => "tick_persistence",
            NodeId::BarPersistence => "bar_persistence",
            NodeId::Timebar1m => "timebar_60s",
            NodeId::Timebar1s => "timebar_1s",
            NodeId::TibsSmall => "tibs_small",
            NodeId::TibsLarge => "tibs_large",
            NodeId::TrbsSmall => "trbs_small",
            NodeId::TrbsLarge => "trbs_large",
            NodeId::VibsSmall => "vibs_small",
            NodeId::VibsLarge => "vibs_large",
            NodeId::Vpin => "vpin",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Collector,
    Persistence,
    Aggregator,
}

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub id: NodeId,
    pub kind: NodeKind,
}

#[derive(Clone, Debug)]
pub struct EdgeSpec {
    pub from: NodeId,
    pub to: NodeId,
    pub stream: StreamKind,
}

#[derive(Clone, Debug)]
pub struct PipelineSpec {
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
}

impl Default for PipelineSpec {
    fn default() -> Self {
        // NOTE: Edges are upstream -> downstream (producer -> consumer).
        // Start order will be derived as "downstream first, upstream last".
        let nodes = vec![
            NodeSpec {
                id: NodeId::CollectorBinanceSpot,
                kind: NodeKind::Collector,
            },
            NodeSpec {
                id: NodeId::CollectorBinanceFutures,
                kind: NodeKind::Collector,
            },
            NodeSpec {
                id: NodeId::TickPersistence,
                kind: NodeKind::Persistence,
            },
            NodeSpec {
                id: NodeId::BarPersistence,
                kind: NodeKind::Persistence,
            },
            NodeSpec {
                id: NodeId::Timebar1m,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::Timebar1s,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::TibsSmall,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::TibsLarge,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::TrbsSmall,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::TrbsLarge,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::VibsSmall,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::VibsLarge,
                kind: NodeKind::Aggregator,
            },
            NodeSpec {
                id: NodeId::Vpin,
                kind: NodeKind::Aggregator,
            },
        ];

        let mut edges = Vec::new();
        for collector in [
            NodeId::CollectorBinanceSpot,
            NodeId::CollectorBinanceFutures,
        ] {
            // Collector produces raw streams; tick persistence consumes them.
            edges.push(EdgeSpec {
                from: collector,
                to: NodeId::TickPersistence,
                stream: StreamKind::Trade,
            });
            edges.push(EdgeSpec {
                from: collector,
                to: NodeId::TickPersistence,
                stream: StreamKind::Orderbook,
            });

            // Collector -> aggregators
            for agg in [
                NodeId::Timebar1m,
                NodeId::Timebar1s,
                NodeId::TibsSmall,
                NodeId::TibsLarge,
                NodeId::TrbsSmall,
                NodeId::TrbsLarge,
                NodeId::VibsSmall,
                NodeId::VibsLarge,
                NodeId::Vpin,
            ] {
                edges.push(EdgeSpec {
                    from: collector,
                    to: agg,
                    stream: StreamKind::Trade,
                });
            }
        }

        // Aggregators -> bar persistence (candle sink)
        for agg in [
            NodeId::Timebar1m,
            NodeId::Timebar1s,
            NodeId::TibsSmall,
            NodeId::TibsLarge,
            NodeId::TrbsSmall,
            NodeId::TrbsLarge,
            NodeId::VibsSmall,
            NodeId::VibsLarge,
            NodeId::Vpin,
        ] {
            edges.push(EdgeSpec {
                from: agg,
                to: NodeId::BarPersistence,
                stream: StreamKind::Candle,
            });
        }

        Self { nodes, edges }
    }
}

impl PipelineSpec {
    pub fn node(&self, id: NodeId) -> Option<&NodeSpec> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Required streams to start/stop on a node for the given venue, derived from edges + node kind.
    ///
    /// Rules:
    /// - Collector: start streams it produces (outgoing edges).
    /// - Aggregator: start streams it produces (outgoing edges). Inputs are implicit (e.g. it subscribes to trades internally).
    /// - Persistence: start streams it consumes (incoming edges).
    pub fn required_streams_for_node(&self, node: NodeId, venue: &VenueId) -> Vec<StreamKind> {
        let Some(n) = self.node(node) else {
            return vec![];
        };

        let allowed_nodes: HashSet<NodeId> = self.nodes_for_venue(venue).into_iter().collect();

        let mut set: HashSet<StreamKind> = HashSet::new();
        match n.kind {
            NodeKind::Collector | NodeKind::Aggregator => {
                for e in &self.edges {
                    if e.from == node && allowed_nodes.contains(&e.to) {
                        set.insert(e.stream);
                    }
                }
            }
            NodeKind::Persistence => {
                for e in &self.edges {
                    if e.to == node && allowed_nodes.contains(&e.from) {
                        set.insert(e.stream);
                    }
                }
            }
        }

        // Stable ordering for output / execution.
        let mut out = Vec::new();
        for k in [StreamKind::Trade, StreamKind::Orderbook, StreamKind::Candle] {
            if set.contains(&k) {
                out.push(k);
            }
        }
        out
    }

    /// Pick the collector node for a given venue.
    pub fn collector_for_venue(venue: &VenueId) -> NodeId {
        match venue {
            VenueId::BinanceFutures => NodeId::CollectorBinanceFutures,
            _ => NodeId::CollectorBinanceSpot,
        }
    }

    /// Nodes required for the given venue (includes chosen collector and all non-collector nodes).
    pub fn nodes_for_venue(&self, venue: &VenueId) -> Vec<NodeId> {
        let collector = Self::collector_for_venue(venue);
        let mut ids: Vec<NodeId> = self
            .nodes
            .iter()
            .map(|n| n.id)
            .filter(|id| {
                // Include only the collector relevant for this venue.
                match id {
                    NodeId::CollectorBinanceSpot | NodeId::CollectorBinanceFutures => {
                        *id == collector
                    }
                    _ => true,
                }
            })
            .collect();

        // Stable-ish ordering (by label) before topo.
        ids.sort_by_key(|id| id.label());
        ids
    }

    /// Derive start order for a venue: downstream-first, collector-last.
    pub fn start_order_for_venue(&self, venue: &VenueId) -> Result<Vec<NodeId>, String> {
        let nodes = self.nodes_for_venue(venue);

        // We want: for each edge (from -> to), start `to` BEFORE `from`.
        // So we topo-sort a graph with reversed edges: (to -> from).
        let mut indeg: HashMap<NodeId, usize> = nodes.iter().map(|n| (*n, 0)).collect();
        let mut adj: HashMap<NodeId, Vec<NodeId>> = nodes.iter().map(|n| (*n, vec![])).collect();

        for e in &self.edges {
            if !indeg.contains_key(&e.from) || !indeg.contains_key(&e.to) {
                continue;
            }
            // reversed: to -> from
            adj.get_mut(&e.to).unwrap().push(e.from);
            *indeg.get_mut(&e.from).unwrap() += 1;
        }

        let mut q: VecDeque<NodeId> = indeg
            .iter()
            .filter_map(|(k, v)| if *v == 0 { Some(*k) } else { None })
            .collect();
        // Deterministic-ish: smallest label first.
        let mut q_vec: Vec<NodeId> = q.drain(..).collect();
        q_vec.sort_by_key(|id| id.label());
        q = q_vec.into();

        let mut out = Vec::with_capacity(nodes.len());
        while let Some(n) = q.pop_front() {
            out.push(n);
            for m in adj.get(&n).cloned().unwrap_or_default() {
                let d = indeg.get_mut(&m).unwrap();
                *d -= 1;
                if *d == 0 {
                    q.push_back(m);
                }
            }
        }

        if out.len() != nodes.len() {
            return Err("Pipeline graph has a cycle (cannot derive start order)".to_string());
        }
        Ok(out)
    }

    pub fn stop_order_for_venue(&self, venue: &VenueId) -> Result<Vec<NodeId>, String> {
        let mut start = self.start_order_for_venue(venue)?;
        start.reverse();
        Ok(start)
    }
}

pub fn service_spec_by_id(settings: &Settings, id: &str) -> Option<ServiceSpec> {
    // ServiceSpec is Clone in registry output (owned), so we can just search.
    service_registry::all_services(settings)
        .into_iter()
        .find(|s| s.id == id)
}
