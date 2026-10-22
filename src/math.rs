//! Small 2D math helpers. The whole world is flat, so a `Vec2` is all we need.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn from_angle(a: f32) -> Self {
        Self::new(a.cos(), a.sin())
    }

    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn dist(self, other: Self) -> f32 {
        (self - other).len()
    }

    pub fn norm(self) -> Self {
        let len = self.len();
        if len > 1e-6 {
            self * (1.0 / len)
        } else {
            Self::default()
        }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn perp(self) -> Self {
        Self::new(-self.y, self.x)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        *self = *self + rhs;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        *self = *self - rhs;
    }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Cheap deterministic PRNG (xorshift64*). Good enough for shotgun spread.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    pub fn f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lerp(lo, hi, self.f32())
    }
}

/// Stable value noise used by the procedural textures.
pub fn noise(x: i32, y: i32, seed: u32) -> u8 {
    let mut h = (x as u32).wrapping_mul(0x27D4_EB2D)
        ^ (y as u32).wrapping_mul(0x1656_67B1)
        ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_basics() {
        let a = Vec2::new(3.0, 4.0);
        assert!((a.len() - 5.0).abs() < 1e-6);
        assert!((a.norm().len() - 1.0).abs() < 1e-6);
        assert_eq!(a.perp(), Vec2::new(-4.0, 3.0));
        assert_eq!(a.dot(Vec2::new(1.0, 0.0)), 3.0);
    }

    #[test]
    fn rng_stays_in_range() {
        let mut rng = Rng::new(7);
        for _ in 0..1000 {
            let v = rng.range(-0.2, 0.4);
            assert!((-0.2..=0.4).contains(&v));
        }
    }
}
