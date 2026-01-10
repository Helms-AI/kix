//! Knowledge Indexer MCP Server - Model Context Protocol server for the Knowledge Indexer.
//!
//! This crate provides an MCP server that exposes search and retrieval
//! tools to AI assistants.

pub mod error;
pub mod server;
pub mod tools;

pub use error::McpError;
pub use server::{KixMcpServer, EipMcpServer};
