//! Supervisor Desktop architecture.
//!
//! Dependency direction is intentionally one-way:
//! `presentation -> viewmodels -> services -> infrastructure`.
#![allow(dead_code)]

pub mod infrastructure;
pub mod models;
pub mod presentation;
pub mod services;
pub mod viewmodels;
