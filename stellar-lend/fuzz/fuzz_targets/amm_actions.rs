#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    stellarlend_contract_fuzz::amm::run(data);
});
