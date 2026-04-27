/// Fixed-size action encoding used by the fuzzers.
///
/// The fuzzer input is interpreted as `N` consecutive `ActionBytes` chunks.
/// This keeps the mutation surface "structured" while still allowing libFuzzer
/// to freely mutate bytes and lengths.
pub const ACTION_BYTES_LEN: usize = 32;

#[derive(Clone, Copy)]
pub struct ActionBytes(pub [u8; ACTION_BYTES_LEN]);

impl ActionBytes {
    #[inline]
    pub fn kind(&self) -> u8 {
        self.0[0]
    }

    #[inline]
    pub fn user(&self) -> u8 {
        self.0[1]
    }

    #[inline]
    pub fn asset_a(&self) -> u8 {
        self.0[2]
    }

    #[inline]
    pub fn asset_b(&self) -> u8 {
        self.0[3]
    }

    #[inline]
    pub fn u32_param(&self) -> u32 {
        u32::from_le_bytes([self.0[4], self.0[5], self.0[6], self.0[7]])
    }

    #[inline]
    pub fn i64_a(&self) -> i64 {
        i64::from_le_bytes([
            self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14],
            self.0[15],
        ])
    }

    #[inline]
    pub fn i64_b(&self) -> i64 {
        i64::from_le_bytes([
            self.0[16], self.0[17], self.0[18], self.0[19], self.0[20], self.0[21], self.0[22],
            self.0[23],
        ])
    }

    #[inline]
    pub fn u64_tail(&self) -> u64 {
        u64::from_le_bytes([
            self.0[24], self.0[25], self.0[26], self.0[27], self.0[28], self.0[29], self.0[30],
            self.0[31],
        ])
    }
}

pub fn parse_actions(data: &[u8], max_actions: usize) -> impl Iterator<Item = ActionBytes> + '_ {
    let max_bytes = max_actions.saturating_mul(ACTION_BYTES_LEN);
    let data = &data[..data.len().min(max_bytes)];
    let n = data.len() / ACTION_BYTES_LEN;
    (0..n).map(move |i| {
        let start = i * ACTION_BYTES_LEN;
        let end = start + ACTION_BYTES_LEN;
        let mut chunk = [0u8; ACTION_BYTES_LEN];
        chunk.copy_from_slice(&data[start..end]);
        ActionBytes(chunk)
    })
}
