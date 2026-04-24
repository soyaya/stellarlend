#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    stellarlend_contract_fuzz::lending::run(data);
});

// ─────────────────────────────────────────────────────────────────────────────
// Custom libFuzzer mutator (protocol-aware)
// ─────────────────────────────────────────────────────────────────────────────
//
// The lending fuzzer uses fixed-size 32-byte "actions". A custom mutator keeps
// inputs aligned to action boundaries and performs small, field-aware tweaks so
// the fuzzer can explore deeper protocol states than raw byte-level mutation.

const ACTION_BYTES_LEN: usize = stellarlend_contract_fuzz::encoding::ACTION_BYTES_LEN;
const MAX_ACTIONS: usize = 64;

#[inline]
fn clamp_size(size: usize, max_size: usize) -> usize {
    let max = max_size.min(MAX_ACTIONS * ACTION_BYTES_LEN);
    let min = ACTION_BYTES_LEN;
    let size = size.clamp(min, max);
    size - (size % ACTION_BYTES_LEN)
}

#[inline]
fn xorshift32(mut x: u32) -> u32 {
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

#[no_mangle]
pub extern "C" fn LLVMFuzzerCustomMutator(
    data: *mut u8,
    size: usize,
    max_size: usize,
    seed: u32,
) -> usize {
    if data.is_null() || max_size < ACTION_BYTES_LEN {
        return 0;
    }

    // Keep inputs action-aligned.
    let mut size = clamp_size(size, max_size);

    unsafe {
        let slice = core::slice::from_raw_parts_mut(data, max_size);
        let actions = size / ACTION_BYTES_LEN;

        let mut rng = seed ^ 0xA5A5_5A5A;
        let iters = 1 + (seed as usize % 8);
        for _ in 0..iters {
            rng = xorshift32(rng);
            let pick = (rng as usize) % actions.max(1);
            let base = pick * ACTION_BYTES_LEN;

            // Choose a field to mutate (biased towards "semantic" bytes).
            rng = xorshift32(rng);
            match rng % 6 {
                // kind / user / asset selectors
                0 => slice[base] = slice[base].wrapping_add((rng as u8) | 1),
                1 => slice[base + 1] = slice[base + 1].wrapping_add((rng as u8) | 1),
                2 => slice[base + 2] = slice[base + 2].wrapping_add((rng as u8) | 1),
                3 => slice[base + 3] = slice[base + 3].wrapping_add((rng as u8) | 1),
                // amount tweak (little-endian i64 at +8)
                4 => {
                    let off = base + 8 + ((rng as usize) % 8);
                    slice[off] ^= (rng as u8).rotate_left(1) | 0x1;
                }
                // time / param tweak
                _ => {
                    let off = base + 24 + ((rng as usize) % 8);
                    slice[off] ^= (rng as u8).rotate_left(3) | 0x1;
                }
            }
        }

        // Occasionally grow/shrink by exactly one action (keeps structure).
        rng = xorshift32(rng);
        if (rng & 0xF) == 0 && actions > 1 {
            // shrink
            size -= ACTION_BYTES_LEN;
        } else if (rng & 0xF) == 1 && (size + ACTION_BYTES_LEN) <= max_size {
            // grow (initialize new bytes from a simple PRNG)
            let new_base = size;
            for i in 0..ACTION_BYTES_LEN {
                rng = xorshift32(rng);
                slice[new_base + i] = (rng & 0xFF) as u8;
            }
            size += ACTION_BYTES_LEN;
        }
    }

    size
}
