const SHOW_GAUGES_KEY = "pokkepet.showGauges";

// localStorage は無効化されている場合があるので、失敗しても動作は続ける。
export function loadShowGauges(): boolean {
  try {
    return localStorage.getItem(SHOW_GAUGES_KEY) === "1";
  } catch {
    return false;
  }
}

export function saveShowGauges(show: boolean): void {
  try {
    localStorage.setItem(SHOW_GAUGES_KEY, show ? "1" : "0");
  } catch {
    // 保存できなくても今回の表示切り替えには影響しない。
  }
}
