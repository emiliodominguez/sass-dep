//! Output schema and serialization.
//!
//! - [`OutputSchema`] is the versioned, serializable representation of an
//!   analyzed graph (the JSON document produced by `sass-dep analyze`).
//! - [`Serializer`] renders a schema as JSON, Graphviz DOT, Mermaid or D2.

mod schema;
mod serializer;

pub use schema::{
    Analysis, Metadata, OutputEdge, OutputNode, OutputSchema, Statistics, SCHEMA_URL,
    SCHEMA_VERSION,
};
pub use serializer::Serializer;
