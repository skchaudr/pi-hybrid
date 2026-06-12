//! ts-bridge RPC interface.
//!
//! Provides convenience re-exports and constructors for JSON-RPC communication
//! with the TypeScript Pi process.
//!
//! The core bridge types (TsBridge, SkillInfo, CallSkillResult, etc.)
//! are defined in the crate root at `lib.rs`.

// Re-export key types from the crate root for ergonomic use.
pub use crate::{
    CallSkillResult, JsonRpcRequest, JsonRpcResponse, RegisterToolArgs, SendPromptArgs, SkillInfo,
    TokenChunk, TsBridge,
};
