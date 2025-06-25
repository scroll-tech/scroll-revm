#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc as std;

pub mod builder;

pub use evm::ScrollEvm;
pub mod evm;

mod exec;

pub mod handler;

pub mod instructions;

pub mod l1block;

pub mod precompile;

pub use spec::*;
mod spec;

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub use transaction::ScrollTransaction;
mod transaction;
