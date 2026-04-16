//! Humanization Layer - Behavioral Mutation Middleware
//!
//! Injects "real human" characteristics into machine-executed browser actions.
//! Placed between the Template Engine (action sequences) and the Browser Executor (CDP calls).

pub mod config;
pub mod failure;
pub mod middleware;
pub mod scroll;
pub mod timing;
pub mod trajectory;
pub mod typing;

pub use config::{HumanizationConfig, HumanizationLevel};
pub use middleware::BehavioralMutationMiddleware;
