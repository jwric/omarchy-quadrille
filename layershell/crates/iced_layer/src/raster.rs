//! Retained native rectangles. Each released shm buffer catches up from its
//! own previous frame; unchanged marks never enter the paint path.
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterRect {
    pub bounds: [i32; 4],
    pub pixel: [u8; 4],
    /// Glyphs in a plate share its repaint region.
    pub damage_bounds: Option<[i32; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterBuffer {
    pub size: (u32, u32),
    pub rects: Arc<[RasterRect]>,
}

fn intersection(a: [i32; 4], b: [i32; 4]) -> Option<[i32; 4]> {
    let x = a[0].max(b[0]);
    let y = a[1].max(b[1]);
    let w = (a[0] + a[2]).min(b[0] + b[2]) - x;
    let h = (a[1] + a[3]).min(b[1] + b[3]) - y;
    (w > 0 && h > 0).then_some([x, y, w, h])
}

impl RasterBuffer {
    pub fn byte_len(&self) -> usize {
        self.size.0 as usize * self.size.1 as usize * 4
    }

    pub fn damage(&self, previous: Option<&Self>) -> Vec<[i32; 4]> {
        let full = [0, 0, self.size.0 as i32, self.size.1 as i32];
        let Some(old) = previous.filter(|old| old.size == self.size) else {
            return vec![full];
        };
        let mut damage = Vec::new();
        for i in 0..self.rects.len().max(old.rects.len()) {
            let a = old.rects.get(i);
            let b = self.rects.get(i);
            if a == b {
                continue;
            }
            for rect in [a, b].into_iter().flatten() {
                if let Some(bounds) = intersection(rect.damage_bounds.unwrap_or(rect.bounds), full)
                {
                    damage.push(bounds);
                }
            }
        }
        // Merge only rectangles sharing an axis and extent. Bounding boxes
        // around crossing hairlines would damage most of the output.
        damage.sort_unstable();
        damage.dedup();
        let mut merged: Vec<[i32; 4]> = Vec::new();
        for b in damage {
            if let Some(a) = merged.iter_mut().find(|a| {
                (a[0] == b[0] && a[2] == b[2] && a[1] <= b[1] + b[3] && b[1] <= a[1] + a[3])
                    || (a[1] == b[1] && a[3] == b[3] && a[0] <= b[0] + b[2] && b[0] <= a[0] + a[2])
            }) {
                let x = a[0].min(b[0]);
                let y = a[1].min(b[1]);
                *a = [
                    x,
                    y,
                    (a[0] + a[2]).max(b[0] + b[2]) - x,
                    (a[1] + a[3]).max(b[1] + b[3]) - y,
                ];
            } else {
                merged.push(b);
            }
        }
        merged
    }

    pub fn draw(&self, canvas: &mut [u8], previous: Option<&Self>) {
        let damage = self.damage(previous);
        let stride = self.size.0 as usize * 4;
        for &[x, y, w, h] in &damage {
            for yy in y..y + h {
                let start = yy as usize * stride + x as usize * 4;
                canvas[start..start + w as usize * 4].fill(0);
            }
        }
        for rect in self.rects.iter() {
            for area in &damage {
                if let Some([x, y, w, h]) = intersection(rect.bounds, *area) {
                    for yy in y..y + h {
                        let start = yy as usize * stride + x as usize * 4;
                        for pixel in canvas[start..start + w as usize * 4].chunks_exact_mut(4) {
                            pixel.copy_from_slice(&rect.pixel);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_matches_full_paint_even_with_overlap_and_skipped_frames() {
        let mut slots = [vec![0; 200 * 100 * 4], vec![0; 200 * 100 * 4]];
        let mut previous = [None, None];
        for x in [10, 12, 50, 199, -5, 30] {
            let frame = RasterBuffer {
                size: (200, 100),
                rects: vec![
                    RasterRect {
                        bounds: [5, 5, 180, 1],
                        pixel: [1, 2, 3, 255],
                        damage_bounds: None,
                    },
                    RasterRect {
                        bounds: [x, 0, 1, 100],
                        pixel: [4, 5, 6, 255],
                        damage_bounds: None,
                    },
                    RasterRect {
                        bounds: [x - 4, 3, 20, 16],
                        pixel: [7, 8, 9, 255],
                        damage_bounds: Some([x - 4, 3, 20, 16]),
                    },
                ]
                .into(),
            };
            let mut full = vec![0; frame.byte_len()];
            frame.draw(&mut full, None);
            let slot = (x as usize) % 2;
            frame.draw(&mut slots[slot], previous[slot].as_ref());
            assert_eq!(slots[slot], full);
            if let Some(old) = &previous[slot] {
                let damage = frame.damage(Some(old));
                assert!(damage.iter().map(|r| r[2] * r[3]).sum::<i32>() < 200 * 100 / 2);
            }
            assert!(frame.damage(Some(&frame)).is_empty());
            previous[slot] = Some(frame);
        }
    }
}
