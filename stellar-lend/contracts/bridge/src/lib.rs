#![no_std]
#![allow(deprecated)]
mod bridge;

#[cfg(any(test, feature = "testutils"))]
pub use bridge::BridgeContractClient;
pub use bridge::{BridgeContract, ContractError};

#[cfg(test)]
mod math_safety_test;
#[cfg(test)]
mod test;
