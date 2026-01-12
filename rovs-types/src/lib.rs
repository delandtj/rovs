//! Shared types for OVSDB and OpenFlow protocols.
//!
//! This crate provides the fundamental data types used across the rovs ecosystem:
//! - `Atom` - OVSDB atomic values (integers, strings, UUIDs, etc.)
//! - `Datum` - OVSDB collections (sets and maps of atoms)
//! - `OvsType` - Type definitions with constraints
//! - Common error types

mod atom;
mod datum;
mod error;

pub use atom::Atom;
pub use datum::Datum;
pub use error::Error;

pub type Result<T> = std::result::Result<T, Error>;
