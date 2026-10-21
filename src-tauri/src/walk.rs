//! おでかけ中に、デスクトップの上を歩き回るペットの動き（位置の計算だけ。ウィンドウの操作は shell が行う）。
//! 単位はポイント（論理ピクセル）。

use serde::Serialize;

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

/// ペットのしぐさ。画面側がアニメーションを出す。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// クリックされて、ぴょんと跳ねて喜ぶ。
    Hop,
    /// ダブルクリックされて、くるっと回る。
    Spin,
    Yawn,
    Stretch,
}

impl Action {
    /// そのしぐさの長さ（秒）。この間は立ち止まる。
    fn secs(self) -> f64 {
        match self {
            Action::Hop => 0.6,
            Action::Spin => 0.8,
            Action::Yawn => 1.6,
            Action::Stretch => 1.4,
        }
    }
}

/// 目的地に着いたとき、あくびか伸びをする確率。
const IDLE_ACTION_CHANCE: f64 = 0.4;
/// 重力（ポイント/秒²）。
const GRAVITY: f64 = 2000.0;
/// 壁・床で跳ね返るときに残る速さの割合。
const WALL_BOUNCE: f64 = 0.6;
const FLOOR_BOUNCE: f64 = 0.4;
/// 床でこれより遅くなったら、着地とみなす。
const LANDING_SPEED: f64 = 150.0;
/// つかんで、これだけ動かしたら「ドラッグ」（それまでは「クリック」）。
const DRAG_MIN_PX: f64 = 4.0;
/// 手を離したときの速さを求めるために、何秒ぶんの動きを覚えておくか。
const DRAG_SAMPLE_SECS: f64 = 0.1;
/// 投げたときの速さの上限。
const THROW_MAX_SPEED: f64 = 3000.0;

/// 1 コマぶんの結果。`dir` は進む向き（-1: 左, 1: 右）。`seq` は、しぐさが始まるたびに増える（同じしぐさを続けて出すため）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub dir: i8,
    pub moving: bool,
    /// つかまれている。
    pub held: bool,
    /// 投げられて、飛んでいる（落ちている）。
    pub flying: bool,
    pub action: Option<Action>,
    pub seq: u32,
}

enum Mode {
    Walk,
    /// つかまれている。`start` はつかんだ位置、`off` はつかんだ位置とペットの左上のずれ。
    Held { start: (f64, f64), off: (f64, f64), moved: bool, samples: Vec<(f64, (f64, f64))> },
    /// 投げられた・落ちている。
    Fly { vx: f64, vy: f64 },
}

pub struct Walker {
    x: f64,
    y: f64,
    target: (f64, f64),
    rest: f64,
    dir: i8,
    mode: Mode,
    clock: f64,
    action: Option<Action>,
    action_left: f64,
    seq: u32,
}

fn unit(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

fn pick_point(areas: &[Area], rng: &mut Rng) -> Option<(f64, f64)> {
    let area = areas.get(rng.next_below(areas.len() as u64) as usize)?;
    Some((area.x + area.w * unit(rng), area.y + area.h * unit(rng)))
}

/// 位置 `p` の下にある範囲（横が重なるもの。なければ最初のもの）。
fn bounds_for(areas: &[Area], p: (f64, f64)) -> Option<Area> {
    areas.iter().find(|a| p.0 >= a.x && p.0 <= a.x + a.w).or(areas.first()).copied()
}

impl Walker {
    /// 範囲のどこかから歩き出す。歩ける範囲がなければ None。
    pub fn new(areas: &[Area], rng: &mut Rng) -> Option<Self> {
        let (x, y) = pick_point(areas, rng)?;
        Some(Self { x, y, target: (x, y), rest: 0.0, dir: 1, mode: Mode::Walk, clock: 0.0, action: None, action_left: 0.0, seq: 0 })
    }

    fn frame(&self, moving: bool) -> Frame {
        Frame {
            x: self.x,
            y: self.y,
            dir: self.dir,
            moving,
            held: matches!(self.mode, Mode::Held { moved: true, .. }),
            flying: matches!(self.mode, Mode::Fly { .. }),
            action: self.action.filter(|_| self.action_left > 0.0),
            seq: self.seq,
        }
    }

    /// しぐさを始める（その間は立ち止まる）。
    pub fn act(&mut self, action: Action) {
        self.seq = self.seq.wrapping_add(1);
        self.action = Some(action);
        self.action_left = action.secs();
        self.rest = self.rest.max(action.secs());
    }

    /// マウスのボタンが押された（`cursor` は画面上のマウスの位置）。
    pub fn grab(&mut self, cursor: (f64, f64)) {
        self.mode = Mode::Held { start: cursor, off: (cursor.0 - self.x, cursor.1 - self.y), moved: false, samples: vec![(self.clock, (self.x, self.y))] };
    }

    /// つかんでいる間、マウスについていく（少し動かすまでは、クリックのつもりかもしれないので動かさない）。
    pub fn drag(&mut self, cursor: (f64, f64)) {
        let clock = self.clock;
        let Mode::Held { start, off, moved, samples } = &mut self.mode else {
            return;
        };
        if !*moved && (cursor.0 - start.0).hypot(cursor.1 - start.1) > DRAG_MIN_PX {
            *moved = true;
        }
        if *moved {
            self.x = cursor.0 - off.0;
            self.y = cursor.1 - off.1;
            samples.push((clock, (self.x, self.y)));
            samples.retain(|&(t, _)| clock - t <= DRAG_SAMPLE_SECS);
        }
    }

    /// マウスのボタンが離された。動かしていなければクリック（跳ねて喜ぶ）。動かしていたら、手を離した勢いで飛ぶ（落ちる）。
    pub fn release(&mut self) {
        let Mode::Held { moved, samples, .. } = std::mem::replace(&mut self.mode, Mode::Walk) else {
            return;
        };
        if !moved {
            self.act(Action::Hop);
            return;
        }
        let (vx, vy) = match (samples.first(), samples.last()) {
            (Some(&(t0, (x0, y0))), Some(&(t1, (x1, y1)))) if t1 - t0 > 1e-6 => ((x1 - x0) / (t1 - t0), (y1 - y0) / (t1 - t0)),
            _ => (0.0, 0.0),
        };
        let speed = vx.hypot(vy);
        let k = if speed > THROW_MAX_SPEED { THROW_MAX_SPEED / speed } else { 1.0 };
        self.mode = Mode::Fly { vx: vx * k, vy: vy * k };
    }

    /// つかまれているか。
    pub fn is_held(&self) -> bool {
        matches!(self.mode, Mode::Held { .. })
    }

    /// `dt` 秒ぶん進める。画面の抜き差しで目的地が範囲の外になったら、選び直す。
    pub fn step(&mut self, dt: f64, areas: &[Area], rng: &mut Rng) -> Frame {
        self.step_with(dt, areas, rng, false)
    }

    /// `step` に、マウスがペットの上にあるか（あれば、立ち止まって待つ）を加えたもの。
    pub fn step_with(&mut self, dt: f64, areas: &[Area], rng: &mut Rng, hover: bool) -> Frame {
        self.clock += dt;
        self.action_left = (self.action_left - dt).max(0.0);
        match self.mode {
            Mode::Held { .. } => self.frame(false),
            Mode::Fly { vx, vy } => self.fly(dt, areas, rng, vx, vy),
            Mode::Walk => self.walk(dt, areas, rng, hover),
        }
    }

    fn fly(&mut self, dt: f64, areas: &[Area], rng: &mut Rng, mut vx: f64, mut vy: f64) -> Frame {
        let Some(b) = bounds_for(areas, (self.x, self.y)) else {
            return self.frame(false);
        };
        vy += GRAVITY * dt;
        self.x += vx * dt;
        self.y += vy * dt;
        if vx != 0.0 {
            self.dir = if vx < 0.0 { -1 } else { 1 };
        }
        if self.x < b.x || self.x > b.x + b.w {
            self.x = self.x.clamp(b.x, b.x + b.w);
            vx = -vx * WALL_BOUNCE;
        }
        if self.y < b.y {
            self.y = b.y;
            vy = -vy * WALL_BOUNCE;
        }
        if self.y >= b.y + b.h {
            self.y = b.y + b.h;
            if vy.abs() < LANDING_SPEED {
                self.mode = Mode::Walk;
                self.rest = 1.0;
                self.target = pick_point(areas, rng).unwrap_or((self.x, self.y));
                return self.frame(false);
            }
            vy = -vy * FLOOR_BOUNCE;
            vx *= 0.8;
        }
        self.mode = Mode::Fly { vx, vy };
        self.frame(false)
    }

    fn walk(&mut self, dt: f64, areas: &[Area], rng: &mut Rng, hover: bool) -> Frame {
        if !areas.iter().any(|a| a.contains(self.target)) {
            let Some(target) = pick_point(areas, rng) else {
                return self.frame(false);
            };
            self.target = target;
        }
        if hover {
            return self.frame(false);
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
            if unit(rng) < IDLE_ACTION_CHANCE {
                self.act(if unit(rng) < 0.5 { Action::Yawn } else { Action::Stretch });
            }
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

    fn walker_at(x: f64, y: f64) -> (Walker, Vec<Area>, Rng) {
        let a = areas();
        let mut rng = Rng::new(21);
        let mut w = Walker::new(&a, &mut rng).unwrap();
        (w.x, w.y) = (x, y);
        w.target = (x, y);
        (w, a, rng)
    }

    #[test]
    fn a_click_makes_it_hop_and_stand_still_for_a_moment() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.grab((310.0, 210.0));
        assert!(w.is_held());
        w.drag((311.0, 211.0)); // 4px 以内は、クリックのつもり
        w.release();
        assert!(!w.is_held());
        let f = w.step(0.1, &a, &mut rng);
        assert_eq!((f.x, f.y), (300.0, 200.0), "a click does not move it");
        assert_eq!(f.action, Some(Action::Hop));
        assert!(!f.moving);
        let seq = f.seq;
        for _ in 0..10 {
            w.step(0.1, &a, &mut rng);
        }
        assert_eq!(w.step(0.1, &a, &mut rng).action, None, "the hop is over");
        w.act(Action::Hop);
        assert_ne!(w.frame(false).seq, seq, "the next hop is told apart");
    }

    #[test]
    fn a_double_click_makes_it_spin() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.act(Action::Spin);
        assert_eq!(w.step(0.1, &a, &mut rng).action, Some(Action::Spin));
        assert_eq!(serde_json::to_value(Action::Spin).unwrap(), "spin");
    }

    #[test]
    fn it_follows_the_mouse_while_held_and_stands_still() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.grab((310.0, 210.0));
        w.drag((510.0, 310.0));
        let f = w.step(0.033, &a, &mut rng);
        assert_eq!((f.x, f.y), (500.0, 300.0));
        assert!(f.held && !f.moving && !f.flying);
        let again = w.step(0.033, &a, &mut rng);
        assert_eq!((again.x, again.y), (500.0, 300.0), "it does not wander off");
    }

    #[test]
    fn dragging_without_holding_does_nothing_and_releasing_without_holding_too() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.target = (800.0, 200.0);
        w.drag((999.0, 999.0));
        w.release();
        let f = w.step(0.1, &a, &mut rng);
        assert!(f.moving && f.action.is_none() && f.x > 300.0);
    }

    #[test]
    fn letting_go_in_the_air_makes_it_fall_to_the_floor_and_land() {
        let (mut w, a, mut rng) = walker_at(300.0, 100.0);
        w.grab((310.0, 110.0));
        for i in 1..=10 {
            w.drag((310.0 + i as f64 * 10.0, 110.0));
            w.step(0.033, &a, &mut rng);
        }
        w.drag((410.0, 110.0));
        w.step(0.2, &a, &mut rng); // 止まってから離す（勢いなし）
        w.drag((410.0, 110.0));
        w.release();
        assert!(w.frame(false).flying);
        let floor = a[0].y + a[0].h;
        let mut landed = false;
        for _ in 0..600 {
            let f = w.step(0.016, &a, &mut rng);
            assert!(a[0].contains((f.x, f.y)), "{f:?}");
            if !f.flying {
                assert_eq!(f.y, floor);
                landed = true;
                break;
            }
        }
        assert!(landed, "it lands");
    }

    #[test]
    fn a_throw_flies_in_that_direction_bounces_off_the_wall_and_settles() {
        let (mut w, a, mut rng) = walker_at(500.0, 400.0);
        w.grab((510.0, 410.0));
        for i in 1..=5 {
            w.drag((510.0 + i as f64 * 40.0, 410.0));
            w.step(0.016, &a, &mut rng);
        }
        w.release();
        let f = w.step(0.016, &a, &mut rng);
        assert!(f.flying);
        assert_eq!(f.dir, 1, "thrown to the right");
        let right = a[0].x + a[0].w;
        let mut hit_wall = false;
        let mut settled = false;
        for _ in 0..3000 {
            let f = w.step(0.016, &a, &mut rng);
            assert!(a[0].contains((f.x, f.y)), "{f:?}");
            hit_wall |= f.x >= right - 1e-9;
            if !f.flying {
                settled = true;
                break;
            }
        }
        assert!(hit_wall && settled);
    }

    #[test]
    fn a_very_fast_throw_is_capped() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.grab((310.0, 210.0));
        w.drag((320.0, 210.0));
        w.step(0.001, &a, &mut rng);
        w.drag((5000.0, 210.0));
        w.step(0.001, &a, &mut rng);
        w.release();
        let Mode::Fly { vx, vy } = w.mode else { panic!("flying") };
        assert!((vx.hypot(vy) - THROW_MAX_SPEED).abs() < 1e-6);
    }

    #[test]
    fn thrown_upward_it_bounces_off_the_ceiling_and_a_left_throw_faces_left() {
        let (mut w, a, mut rng) = walker_at(500.0, 30.0);
        w.grab((510.0, 40.0));
        for i in 1..=5 {
            w.drag((510.0 - i as f64 * 20.0, 40.0 - i as f64 * 20.0));
            w.step(0.016, &a, &mut rng);
        }
        w.release();
        let f = w.step(0.016, &a, &mut rng);
        assert_eq!(f.dir, -1);
        for _ in 0..3000 {
            let f = w.step(0.016, &a, &mut rng);
            assert!(a[0].contains((f.x, f.y)), "{f:?}");
        }
    }

    #[test]
    fn it_has_nowhere_to_fall_without_a_screen() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.mode = Mode::Fly { vx: 0.0, vy: 0.0 };
        let f = w.step(0.1, &[], &mut rng);
        assert_eq!((f.x, f.y), (300.0, 200.0));
        let _ = a;
    }

    #[test]
    fn the_mouse_over_it_makes_it_stand_still() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.target = (600.0, 200.0);
        let f = w.step_with(1.0, &a, &mut rng, true);
        assert_eq!((f.x, f.moving), (300.0, false));
        let f = w.step_with(1.0, &a, &mut rng, false);
        assert!(f.moving);
    }

    #[test]
    fn a_hop_stops_a_walk_in_progress() {
        let (mut w, a, mut rng) = walker_at(300.0, 200.0);
        w.target = (600.0, 200.0);
        w.step(0.1, &a, &mut rng);
        w.act(Action::Hop);
        let f = w.step(0.1, &a, &mut rng);
        assert!(!f.moving);
    }

    #[test]
    fn it_sometimes_yawns_or_stretches_when_it_arrives() {
        let (mut yawn, mut stretch, mut none) = (false, false, false);
        for seed in 0..60 {
            let a = areas();
            let mut rng = Rng::new(seed);
            let mut w = Walker::new(&a, &mut rng).unwrap();
            w.target = (w.x + 1.0, w.y);
            let f = w.step(1.0, &a, &mut rng);
            yawn |= f.action == Some(Action::Yawn);
            stretch |= f.action == Some(Action::Stretch);
            none |= f.action.is_none();
        }
        assert!(yawn && stretch && none);
    }
}
