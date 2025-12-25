use crate::pipeline::spec::{NodeId, PipelineSpec, StreamKind};

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


