//! Shared safe math helpers for `i128` operations.
//!
//! Soroban contracts should avoid wrapping arithmetic. These helpers centralize
//! checked operations and return module-local errors.

#![allow(unused)]

use soroban_sdk::Env;

pub fn checked_add(a: i128, b: i128) -> Option<i128> {
    a.checked_add(b)
}

pub fn checked_sub(a: i128, b: i128) -> Option<i128> {
    a.checked_sub(b)
}

pub fn checked_mul(a: i128, b: i128) -> Option<i128> {
    a.checked_mul(b)
}

pub fn checked_div(a: i128, b: i128) -> Option<i128> {
    if b == 0 {
        None
    } else {
        a.checked_div(b)
    }
}

