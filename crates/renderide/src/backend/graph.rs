//! Backend-owned render-graph assembly for the renderer's built-in frame topology.

mod main_graph;

pub(crate) use main_graph::{MainGraphPostProcessingResources, build_main_graph_with_resources};
