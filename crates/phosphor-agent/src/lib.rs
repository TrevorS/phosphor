//! The session: the ACP client, the MCP editor-tool server, and the plumbing
//! between them and the store.
//!
//! Review blocks travel over MCP, watch values over ACP (Q6). Both land in the
//! store; no surface reads a transport directly.
//!
//! Owned by `agent`.
