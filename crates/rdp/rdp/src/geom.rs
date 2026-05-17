// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use zeroize::Zeroize;
#[derive(Debug, Copy, Clone)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.w as i32).max(other.x + other.w as i32);
        let y2 = (self.y + self.h as i32).max(other.y + other.h as i32);
        Rect {
            x: x1,
            y: y1,
            w: (x2 - x1) as u32,
            h: (y2 - y1) as u32,
        }
    }
}

impl From<&freerdp_sys::GDI_RGN> for Rect {
    #[allow(clippy::unnecessary_cast)] // Windows/linux/mac differ on INT32 impl
    fn from(rgn: &freerdp_sys::GDI_RGN) -> Self {
        Self {
            x: rgn.x as i32,
            y: rgn.y as i32,
            w: rgn.w as u32,
            h: rgn.h as u32,
        }
    }
}

#[derive(Debug, Copy, Clone, Zeroize)]
pub enum ScreenSize {
    Full,
    Fixed(u32, u32),
}

/// Methods for ScreenSize
/// values returned for Full are default valid sizes for windowed mode
/// after exiting fullscreen, as we don't have access to the actual
/// screen size here for fullscreen
/// Currently, we use a proportional size of 16:9 for fullscreen default
impl ScreenSize {
    pub fn width(&self) -> u32 {
        match self {
            ScreenSize::Full => 1200, // Fallback value, not too small and not too large
            ScreenSize::Fixed(w, _) => *w,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            ScreenSize::Full => 675, // Fallback value, not too small and not too large
            ScreenSize::Fixed(_, h) => *h,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        matches!(self, ScreenSize::Full)
    }

    pub fn get_fixed_size(&self) -> Option<(u32, u32)> {
        match self {
            ScreenSize::Fixed(w, h) => Some((*w, *h)),
            ScreenSize::Full => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_union_overlapping() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        let u = a.union(&b);
        assert_eq!(u.x, 0);
        assert_eq!(u.y, 0);
        assert_eq!(u.w, 15);
        assert_eq!(u.h, 15);
    }

    #[test]
    fn rect_union_disjoint() {
        let a = Rect::new(0, 0, 5, 5);
        let b = Rect::new(10, 10, 5, 5);
        let u = a.union(&b);
        assert_eq!(u.x, 0);
        assert_eq!(u.y, 0);
        assert_eq!(u.w, 15);
        assert_eq!(u.h, 15);
    }

    #[test]
    fn rect_union_same() {
        let a = Rect::new(1, 2, 3, 4);
        let u = a.union(&a);
        assert_eq!(u.x, 1);
        assert_eq!(u.y, 2);
        assert_eq!(u.w, 3);
        assert_eq!(u.h, 4);
    }

    #[test]
    fn screen_size_full() {
        assert!(ScreenSize::Full.is_fullscreen());
        assert_eq!(ScreenSize::Full.width(), 1200);
        assert_eq!(ScreenSize::Full.height(), 675);
        assert!(ScreenSize::Full.get_fixed_size().is_none());
    }

    #[test]
    fn screen_size_fixed() {
        let s = ScreenSize::Fixed(1920, 1080);
        assert!(!s.is_fullscreen());
        assert_eq!(s.width(), 1920);
        assert_eq!(s.height(), 1080);
        assert_eq!(s.get_fixed_size(), Some((1920, 1080)));
    }
}
