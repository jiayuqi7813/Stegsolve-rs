//! 无外部依赖的小型伪随机数发生器（xorshift64*）。
//!
//! 用于随机颜色映射：相同 seed 产生相同结果，便于单元测试复现；UI 侧每次循环
//! 切换随机映射时传入一个新的 seed（例如系统时间）即可获得不同的随机配色。

/// xorshift64* PRNG。
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// 用给定 seed 创建。seed 为 0 时会被替换为一个非零常量。
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// 返回 [0, n) 内的 u32（n 应 > 0）。
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % n as u64) as u32
    }

    /// 返回 0..=255 的字节。
    #[inline]
    pub fn byte(&mut self) -> u32 {
        self.below(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_with_same_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn byte_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            assert!(r.byte() < 256);
        }
    }

    #[test]
    fn zero_seed_is_handled() {
        let mut r = Rng::new(0);
        // 不应 panic 或永远返回 0
        let mut any_nonzero = false;
        for _ in 0..10 {
            if r.next_u64() != 0 {
                any_nonzero = true;
            }
        }
        assert!(any_nonzero);
    }
}
