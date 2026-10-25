//! # NetGene Policy
//!
//! The trust boundary between "an AI suggested this" and "the system did this".
//!
//! - [`action`]: the catalog of actions agents may request, each with the
//!   capability it needs and its risk level.
//! - [`engine`]: evaluates a proposed action for an actor Gene and returns
//!   `Allow`, `Confirm` (a human must approve) or `Deny`.
//! - [`audit`]: append-only, hash-chained, signed log of every decision.
//!
//! The LLM only ever **proposes** an [`Action`]; the [`PolicyEngine`] decides.

pub mod action;
pub mod engine;
pub mod audit;

pub use action::{Action, ActionSpec, Risk, ACTION_CATALOG};
pub use engine::{Decision, Effect, Grant, Policy, PolicyEngine, Rule};
pub use audit::{AuditEntry, AuditLog, AuditRecord};
