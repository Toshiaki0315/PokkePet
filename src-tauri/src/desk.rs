//! デスクトップの上の窓の情報から、ペットが乗れる窓のふち・全画面アプリの判定をする（計算だけ。窓の一覧を読むのは `macwin`）。
//! 座標は macOS のポイント（画面左上が原点）。

/// 長方形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, (px, py): (f64, f64)) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    /// 1pt 程度の誤差を許して、`o` 全体を覆っているか。
    fn covers(&self, o: &Rect) -> bool {
        const EPS: f64 = 1.0;
        self.x <= o.x + EPS && self.y <= o.y + EPS && self.x + self.w >= o.x + o.w - EPS && self.y + self.h >= o.y + o.h - EPS
    }
}

/// 通常のアプリの窓の階層。
pub const NORMAL_LAYER: i64 = 0;
/// メニューバーの階層。
pub const MENU_BAR_LAYER: i64 = 24;
/// 乗る窓のふちは、画面の上端（メニューバー）からこれだけ下にあること。
const MIN_EDGE_BELOW_TOP: f64 = 30.0;
/// 乗る窓の高さの最低（これより低い窓は、タイトルバーだけ・ポップアップなどなので乗らない）。
const MIN_WINDOW_HEIGHT: f64 = 60.0;

/// 画面に出ている窓 1 つ分。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowInfo {
    pub id: i64,
    pub layer: i64,
    pub bounds: Rect,
    /// このアプリ自身の窓か。
    pub own: bool,
}

/// 窓の上端（ペットが乗れるふち）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub id: i64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
}

fn app_windows(windows: &[WindowInfo]) -> impl Iterator<Item = &WindowInfo> {
    windows.iter().filter(|w| w.layer == NORMAL_LAYER && !w.own)
}

/// 指定した画面が、全画面表示・プレゼンテーション中か。
/// ほかのアプリの一番手前の窓が画面全体を覆っていて、しかもメニューバーが消えているときだけ「全画面」とみなす
/// （メニューバーが見えているなら、ふつうのデスクトップ作業中なので隠れない）。
pub fn is_fullscreen(windows: &[WindowInfo], screen: &Rect) -> bool {
    let menu_bar_visible = windows.iter().any(|w| w.layer == MENU_BAR_LAYER && w.bounds.intersects(screen) && w.bounds.h > 0.0);
    if menu_bar_visible {
        return false;
    }
    app_windows(windows).find(|w| w.bounds.intersects(screen)).is_some_and(|w| w.bounds.covers(screen))
}

/// 点 `p` のある画面が全画面表示中か（どの画面にもなければ false）。
pub fn fullscreen_at(windows: &[WindowInfo], screens: &[Rect], p: (f64, f64)) -> bool {
    screens.iter().find(|s| s.contains(p)).is_some_and(|s| is_fullscreen(windows, s))
}

/// ペットが乗れる窓のふち（手前の窓に隠れていない、幅が `min_width` 以上の窓の上端）。
pub fn perch_edges(windows: &[WindowInfo], screens: &[Rect], min_width: f64) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (i, w) in windows.iter().enumerate() {
        if w.layer != NORMAL_LAYER || w.own || w.bounds.w < min_width || w.bounds.h < MIN_WINDOW_HEIGHT {
            continue;
        }
        let top_middle = (w.bounds.x + w.bounds.w / 2.0, w.bounds.y + 1.0);
        let Some(screen) = screens.iter().find(|s| s.contains(top_middle)) else {
            continue;
        };
        if w.bounds.y < screen.y + MIN_EDGE_BELOW_TOP || windows[..i].iter().any(|f| f.layer == NORMAL_LAYER && !f.own && f.bounds.contains(top_middle)) {
            continue;
        }
        edges.push(Edge { id: w.id, x: w.bounds.x, y: w.bounds.y, w: w.bounds.w });
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 0.0, y: 0.0, w: 1800.0, h: 1169.0 };

    fn win(id: i64, layer: i64, bounds: Rect) -> WindowInfo {
        WindowInfo { id, layer, bounds, own: false }
    }

    #[test]
    fn rect_basics() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains((10.0, 20.0)));
        assert!(!r.contains((110.0, 20.0)));
        assert!(r.intersects(&Rect::new(50.0, 50.0, 100.0, 100.0)));
        assert!(!r.intersects(&Rect::new(110.0, 0.0, 10.0, 10.0)));
        assert!(Rect::new(0.5, 0.0, 1799.0, 1169.0).covers(&SCREEN));
        assert!(!Rect::new(0.0, 39.0, 1800.0, 1130.0).covers(&SCREEN));
    }

    #[test]
    fn fullscreen_needs_the_menu_bar_to_be_hidden() {
        let menu_bar = win(1, MENU_BAR_LAYER, Rect::new(0.0, 0.0, 1800.0, 39.0));
        let cover = win(2, NORMAL_LAYER, SCREEN);
        assert!(!is_fullscreen(&[menu_bar, cover], &SCREEN));
        assert!(is_fullscreen(&[cover], &SCREEN));
    }

    #[test]
    fn a_maximized_window_a_covered_one_and_our_own_window_are_not_fullscreen() {
        let maximized = win(2, NORMAL_LAYER, Rect::new(0.0, 39.0, 1800.0, 1130.0));
        assert!(!is_fullscreen(&[maximized], &SCREEN));
        let small = win(2, NORMAL_LAYER, Rect::new(100.0, 100.0, 400.0, 300.0));
        assert!(!is_fullscreen(&[small, win(3, NORMAL_LAYER, SCREEN)], &SCREEN), "only the frontmost window counts");
        let mut mine = win(2, NORMAL_LAYER, SCREEN);
        mine.own = true;
        assert!(!is_fullscreen(&[mine], &SCREEN));
    }

    #[test]
    fn fullscreen_is_asked_about_the_screen_the_pet_is_on() {
        let left = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let right = Rect::new(1000.0, 0.0, 1000.0, 800.0);
        let movie = [win(1, NORMAL_LAYER, right)];
        assert!(fullscreen_at(&movie, &[left, right], (1500.0, 400.0)));
        assert!(!fullscreen_at(&movie, &[left, right], (500.0, 400.0)));
        assert!(!fullscreen_at(&movie, &[left, right], (5000.0, 400.0)), "off every screen");
    }

    #[test]
    fn a_wide_window_not_covered_by_another_has_an_edge_to_stand_on() {
        let w = win(7, NORMAL_LAYER, Rect::new(200.0, 300.0, 800.0, 500.0));
        assert_eq!(perch_edges(&[w], &[SCREEN], 120.0), vec![Edge { id: 7, x: 200.0, y: 300.0, w: 800.0 }]);
    }

    #[test]
    fn narrow_short_menu_own_and_off_screen_windows_have_no_edge() {
        let mut mine = win(4, NORMAL_LAYER, Rect::new(200.0, 300.0, 800.0, 500.0));
        mine.own = true;
        let all = [
            win(1, NORMAL_LAYER, Rect::new(200.0, 300.0, 100.0, 500.0)),   // 狭い
            win(2, NORMAL_LAYER, Rect::new(200.0, 300.0, 800.0, 30.0)),    // 低い
            win(3, MENU_BAR_LAYER, Rect::new(0.0, 0.0, 1800.0, 39.0)),     // メニューバー
            mine,
            win(5, NORMAL_LAYER, Rect::new(200.0, 10.0, 800.0, 500.0)),    // 画面の一番上（メニューバーのすぐ下）
            win(6, NORMAL_LAYER, Rect::new(5000.0, 300.0, 800.0, 500.0)),  // 画面の外
        ];
        assert!(perch_edges(&all, &[SCREEN], 120.0).is_empty());
    }

    #[test]
    fn an_edge_hidden_behind_a_window_in_front_is_skipped() {
        let front = win(1, NORMAL_LAYER, Rect::new(100.0, 250.0, 1000.0, 600.0));
        let back = win(2, NORMAL_LAYER, Rect::new(200.0, 300.0, 800.0, 500.0));
        assert!(perch_edges(&[front, back], &[SCREEN], 120.0).iter().all(|e| e.id == 1));
        assert_eq!(perch_edges(&[front, back], &[SCREEN], 120.0).len(), 1);
    }
}
