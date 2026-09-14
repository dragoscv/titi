//! Time helpers. The core never reads a clock; hosts pass `now_ms`.

/// Milliseconds since Unix epoch, supplied by the host.
pub type Ms = u64;

/// 20 ms voice tick used by `ts16` in the voice header (Serval VoMP style).
pub const TICK_MS: u64 = 20;

/// Invite-code rotation slot (10 minutes).
pub const SLOT_MS: u64 = 10 * 60 * 1000;

#[inline]
pub fn to_tick16(now_ms: Ms) -> u16 {
    ((now_ms / TICK_MS) & 0xFFFF) as u16
}

/// Reconstruct an absolute value from a wrapped 16-bit counter given the
/// last known absolute value (±0x8000 window).
#[inline]
pub fn unwrap16(last_abs: u64, wrapped: u16) -> u64 {
    let base = last_abs & !0xFFFF;
    let low = last_abs & 0xFFFF;
    let w = wrapped as u64;
    let diff = w.wrapping_sub(low) & 0xFFFF;
    if diff < 0x8000 {
        base + low + diff
    } else {
        (base + low).saturating_sub(0x10000 - diff)
    }
}

#[inline]
pub fn slot_index(now_ms: Ms) -> u64 {
    now_ms / SLOT_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_forward_and_backward() {
        assert_eq!(unwrap16(100, 105), 105);
        assert_eq!(unwrap16(0xFFF0, 0x0005), 0x10005);
        assert_eq!(unwrap16(0x10005, 0xFFF0), 0xFFF0);
        assert_eq!(unwrap16(5, 0xFFFF), 0); // saturates instead of underflow
    }
}
