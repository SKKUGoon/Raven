use crate::config::Settings;
use crate::pipeline::spec::{NodeId, PipelineSpec, StreamKind};
use crate::utils::service_registry;

pub enum GraphFormat {
    Ascii,
    Dot,
}

impl GraphFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "ascii" => Some(GraphFormat::Ascii),
            "dot" | "graphviz" => Some(GraphFormat::Dot),
            _ => None,
        }
    }
}

fn kind_label(stream: StreamKind) -> &'static str {
    stream.label()
}

pub fn render_ascii(spec: &PipelineSpec) -> String {
    // Group edges by from-node for a readable adjacency list.
    let mut by_from: Vec<(NodeId, Vec<(NodeId, StreamKind)>)> = spec
        .nodes
        .iter()
        .map(|n| {
            let mut outs: Vec<(NodeId, StreamKind)> = spec
                .edges
                .iter()
                .filter(|e| e.from == n.id)
                .map(|e| (e.to, e.stream))
                .collect();
            outs.sort_by_key(|(to, s)| (to.label(), kind_label(*s)));
            (n.id, outs)
        })
        .collect();
    by_from.sort_by_key(|(from, _)| from.label());

    let mut out = String::new();
    out.push_str("Pipeline graph (upstream -> downstream):\n");
    for (from, outs) in by_from {
        out.push_str(&format!("- {}\n", from.label()));
        for (to, stream) in outs {
            out.push_str(&format!("  -> {} [{}]\n", to.label(), stream.label()));
        }
    }
    out
}

fn render_services_ascii(settings: Option<&Settings>) -> String {
    let mut out = String::new();
    out.push_str("Service graph (process topology):\n");
    match settings {
        Some(s) => {
            let host = service_registry::client_host(&s.server.host);
            let services = service_registry::all_services(s);
            for svc in services {
                out.push_str(&format!(
                    "- {} ({}) @ {}\n",
                    svc.display_name,
                    svc.id,
                    svc.addr(host)
                ));
            }
        }
        None => {
            out.push_str("- unavailable (settings not loaded)\n");
        }
    }
    out
}

pub fn render_ascii_with_scope(
    spec: &PipelineSpec,
    settings: Option<&Settings>,
    show_pipeline: bool,
    show_services: bool,
) -> String {
    let mut out = String::new();
    if show_pipeline {
        out.push_str(&render_ascii(spec));
    }
    if show_services {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&render_services_ascii(settings));
    }
    out
}

pub fn render_dot(spec: &PipelineSpec) -> String {
    let mut out = String::new();
    out.push_str("digraph raven_pipeline {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box, fontname=\"Arial\"];\n");

    // Nodes
    let mut nodes: Vec<NodeId> = spec.nodes.iter().map(|n| n.id).collect();
    nodes.sort_by_key(|n| n.label());
    for n in nodes {
        out.push_str(&format!("  \"{}\";\n", n.label()));
    }

    // Edges (allow duplicate from->to with different stream labels; keep them separate for clarity)
    let mut edges = spec.edges.clone();
    edges.sort_by_key(|e| (e.from.label(), e.to.label(), e.stream.label()));
    for e in edges {
        out.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            e.from.label(),
            e.to.label(),
            e.stream.label()
        ));
    }
    out.push_str("}\n");
    out
}

fn render_services_dot(settings: Option<&Settings>) -> String {
    let mut out = String::new();
    out.push_str("digraph raven_services {\n");
    out.push_str("  rankdir=LR;\n");
    out.push_str("  node [shape=box, fontname=\"Arial\"];\n");
    match settings {
        Some(s) => {
            let host = service_registry::client_host(&s.server.host);
            let mut services = service_registry::all_services(s);
            services.sort_by_key(|svc| svc.id);
            for svc in services {
                out.push_str(&format!(
                    "  \"{}\" [label=\"{}\\n{}\"];\n",
                    svc.id,
                    svc.display_name,
                    svc.addr(host)
                ));
            }
        }
        None => {
            out.push_str("  \"services_unavailable\" [label=\"settings not loaded\"];\n");
        }
    }
    out.push_str("}\n");
    out
}

pub fn render_dot_with_scope(
    spec: &PipelineSpec,
    settings: Option<&Settings>,
    show_pipeline: bool,
    show_services: bool,
) -> String {
    match (show_pipeline, show_services) {
        (true, false) => render_dot(spec),
        (false, true) => render_services_dot(settings),
        (true, true) => {
            let mut out = String::new();
            out.push_str("digraph raven_topology {\n");
            out.push_str("  rankdir=LR;\n");
            out.push_str("  node [shape=box, fontname=\"Arial\"];\n");

            out.push_str("  subgraph cluster_pipeline {\n");
            out.push_str("    label=\"Collection pipeline\";\n");
            out.push_str("    style=rounded;\n");
            let mut nodes: Vec<NodeId> = spec.nodes.iter().map(|n| n.id).collect();
            nodes.sort_by_key(|n| n.label());
            for n in nodes {
                out.push_str(&format!("    \"pipeline:{}\";\n", n.label()));
            }
            let mut edges = spec.edges.clone();
            edges.sort_by_key(|e| (e.from.label(), e.to.label(), e.stream.label()));
            for e in edges {
                out.push_str(&format!(
                    "    \"pipeline:{}\" -> \"pipeline:{}\" [label=\"{}\"];\n",
                    e.from.label(),
                    e.to.label(),
                    e.stream.label()
                ));
            }
            out.push_str("  }\n");

            out.push_str("  subgraph cluster_services {\n");
            out.push_str("    label=\"Service processes\";\n");
            out.push_str("    style=rounded;\n");
            match settings {
                Some(s) => {
                    let host = service_registry::client_host(&s.server.host);
                    let mut services = service_registry::all_services(s);
                    services.sort_by_key(|svc| svc.id);
                    for svc in services {
                        out.push_str(&format!(
                            "    \"service:{}\" [label=\"{}\\n{}\"];\n",
                            svc.id,
                            svc.display_name,
                            svc.addr(host)
                        ));
                    }
                }
                None => {
                    out.push_str("    \"service:unavailable\" [label=\"settings not loaded\"];\n");
                }
            }
            out.push_str("  }\n");
            out.push_str("}\n");
            out
        }
        (false, false) => String::new(),
    }
}
