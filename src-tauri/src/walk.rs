//! おでかけ中に、デスクトップの上を歩き回るペットの動き（位置の計算だけ。ウィンドウの操作は shell が行う）。
//! 単位はポイント（論理ピクセル）。

use crate::minigame::Rng;

/// 歩く速さ（ポイント/秒）。
pub const WALK_SPEED: f64 = 60.0;
/// 目的地に着いてから、次に歩き出すまでの休み（秒）。
pub const REST_MIN_SECS: f64 = 2.0;
pub const REST_MAX_SECS: f64 = 6.0;

/// ウィンドウの左上が動ける範囲（画面の作業領域から、ウィンドウの大きさを引いたもの）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Area {
    fn contains(&self, (px, py): (f64, f64)) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// 画面の作業領域（メニューバーと Dock を除く）から、大きさ `size` のウィンドウが動ける範囲を作る。狭すぎる画面は除く。
pub fn walkable(work_areas: &[(f64, f64, f64, f64)], size: f64) -> Vec<Area> {
    work_areas.iter().filter(|&&(_, _, w, h)| w > size && h > size).map(|&(x, y, w, h)| Area { x, y, w: w - size, h: h - size }).collect()
}

/// デスクトップを歩かせるか: 設定がオンで、ペットがおでかけ中のとき。
pub fn should_walk(enabled: bool, out: bool) -> bool {
    enabled && out
}

/// 1 コマぶんの結果。`dir` は進む向き（-1: 左, 1: 右）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub dir: i8,
    pub moving: bool,
}

pub struct Walker {
    x: f64,
    y: f64,
    target: (f64, f64),
    rest: f64,
    dir: i8,
}

fn unit(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

fn pick_point(areas: &[Area], rng: &mut Rng) -> Option<(f64, f64)> {
    let area = areas.get(rng.next_below(areas.len() as u64) as usize)?;
    Some((area.x + area.w * unit(rng), area.y + area.h * unit(rng)))
}

impl Walker {
    /// 範囲のどこかから歩き出す。歩ける範囲がなければ None。
    pub fn new(areas: &[Area], rng: &mut Rng) -> Option<Self> {
        let (x, y) = pick_point(areas, rng)?;
        Some(Self { x, y, target: (x, y), rest: 0.0, dir: 1 })
    }

    pub fn frame(&self, moving: bool) -> Frame {
        Frame { x: self.x, y: self.y, dir: self.dir, moving }
    }

    /// `dt` 秒ぶん進める。画面の抜き差しで目的地が範囲の外になったら、選び直す。
    pub fn step(&mut self, dt: f64, areas: &[Area], rng: &mut Rng) -> Frame {
        if !areas.iter().any(|a| a.contains(self.target)) {
            let Some(target) = pick_point(areas, rng) else {
                return self.frame(false);
            };
            self.target = target;
        }
        if self.rest > 0.0 {
            self.rest -= dt;
            if self.rest <= 0.0 {
                self.target = pick_point(areas, rng).unwrap_or(self.target);
            }
            return self.frame(false);
        }
        let (dx, dy) = (self.target.0 - self.x, self.target.1 - self.y);
        let distance = dx.hypot(dy);
        let reach = WALK_SPEED * dt;
        if dx.abs() > 0.5 {
            self.dir = if dx < 0.0 { -1 } else { 1 };
        }
        if distance <= reach {
            self.x = self.target.0;
            self.y = self.target.1;
            self.rest = REST_MIN_SECS + (REST_MAX_SECS - REST_MIN_SECS) * unit(rng);
            return self.frame(false);
        }
        self.x += dx / distance * reach;
        self.y += dy / distance * reach;
        self.frame(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn areas() -> Vec<Area> {
        walkable(&[(0.0, 25.0, 1000.0, 700.0)], 100.0)
    }

    #[test]
    fn it_walks_only_when_allowed_and_the_pet_is_out() {
        assert!(should_walk(true, true));
        assert!(!should_walk(false, true));
        assert!(!should_walk(true, false));
    }

    #[test]
    fn the_window_can_move_only_where_it_fits_on_the_work_area() {
        assert_eq!(areas(), vec![Area { x: 0.0, y: 25.0, w: 900.0, h: 600.0 }]);
    }

    #[test]
    fn a_screen_too_small_for_the_window_is_left_out() {
        assert!(walkable(&[(0.0, 0.0, 80.0, 700.0), (0.0, 0.0, 700.0, 100.0)], 100.0).is_empty());
    }

    #[test]
    fn there_is_nothing_to_walk_on_without_a_screen() {
        let mut rng = Rng::new(1);
        assert!(Walker::new(&[], &mut rng).is_none());
    }

    #[test]
    fn it_starts_inside_the_area() {
        let a = areas();
        for seed in 0..50 {
            let w = Walker::new(&a, &mut Rng::new(seed)).unwrap();
            assert!(a[0].contains((w.x, w.y)));
        }
    }

    #[test]
    fn it_walks_toward_the_target_at_a_steady_speed_and_faces_that_way() {
        let a = areas();
        let mut rng = Rng::new(7);
        let mut w = Walker::new(&a, &mut rng).unwrap();
        (w.x, w.y) = (200.0, 100.0);
        let start = w.x;
        w.target = (700.0, 100.0);
        let frame = w.step(1.0, &a, &mut rng);
        assert!(frame.moving);
        assert_eq!(frame.dir, 1);
        assert!((frame.x - start - WALK_SPEED).abs() < 1e-9);
        let before = w.x;
        w.target = (50.0, 100.0);
        let frame = w.step(0.5, &a, &mut rng);
        assert_eq!(frame.dir, -1);
        assert!((before - frame.x - WALK_SPEED * 0.5).abs() < 1e-9);
    }

    #[test]
    fn it_rests_on_arrival_and_then_picks_a_new_target() {
        let a = areas();
        let mut rng = Rng::new(3);
        let mut w = Walker::new(&a, &mut rng).unwrap();
        w.target = (w.x + 10.0, w.y);
        let arrived = w.step(1.0, &a, &mut rng);
        assert!(!arrived.moving);
        assert_eq!(arrived.x, w.target.0);
        assert!((REST_MIN_SECS..=REST_MAX_SECS).contains(&w.rest));
        let reached = w.target;
        assert!(!w.step(1.0, &a, &mut rng).moving, "still resting");
        let mut t = 0.0;
        while !w.step(0.5, &a, &mut rng).moving {
            t += 0.5;
            assert!(t < 10.0, "it starts walking again");
        }
        assert_ne!(w.target, reached);
    }

    #[test]
    fn it_never_leaves_the_area() {
        let a = areas();
        let mut rng = Rng::new(11);
        let mut w = Walker::new(&a, &mut rng).unwrap();
        for _ in 0..5000 {
            let f = w.step(0.033, &a, &mut rng);
            assert!(a[0].contains((f.x, f.y)), "{f:?}");
        }
    }

    #[test]
    fn it_visits_every_screen_when_there_are_several() {
        let a = walkable(&[(0.0, 0.0, 500.0, 400.0), (500.0, 0.0, 500.0, 400.0)], 100.0);
        let mut rng = Rng::new(5);
        let mut w = Walker::new(&a, &mut rng).unwrap();
        let (mut left, mut right) = (false, false);
        for _ in 0..20000 {
            let f = w.step(0.1, &a, &mut rng);
            left |= f.x < 400.0;
            right |= f.x >= 500.0;
        }
        assert!(left && right);
    }

    #[test]
    fn when_the_screen_is_unplugged_it_heads_for_one_that_is_left() {
        let two = walkable(&[(0.0, 0.0, 500.0, 400.0), (2000.0, 0.0, 500.0, 400.0)], 100.0);
        let mut rng = Rng::new(9);
        let mut w = Walker::new(&two, &mut rng).unwrap();
        w.target = (2100.0, 100.0);
        let one = &two[..1];
        w.step(0.1, one, &mut rng);
        assert!(one[0].contains(w.target));
    }

    #[test]
    fn with_no_screen_at_all_it_stays_put() {
        let a = areas();
        let mut rng = Rng::new(2);
        let mut w = Walker::new(&a, &mut rng).unwrap();
        let (x, y) = (w.x, w.y);
        let f = w.step(1.0, &[], &mut rng);
        assert_eq!((f.x, f.y, f.moving), (x, y, false));
    }
}
