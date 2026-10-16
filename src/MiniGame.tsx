import { useState } from "react";
import type { MiniGameReport, Side } from "./api";

const SIDE_NAMES: Record<Side, string> = { left: "左", right: "右" };

interface MiniGameProps {
  /** 選んだ手を送り、結果を返す。 */
  onPlay: (guess: Side) => Promise<MiniGameReport>;
  onClose: () => void;
}

/** 「どっちの手？」: ペットがおやつを隠した手を当てる（画面の中に出す小さなダイアログ）。 */
export function MiniGame({ onPlay, onClose }: MiniGameProps) {
  const [report, setReport] = useState<MiniGameReport | null>(null);

  const choose = (guess: Side) => {
    onPlay(guess).then(setReport, () => {
      // 失敗の表示は呼び出し側（エラー欄）が行う。選び直せるよう、このまま残す。
    });
  };

  return (
    <div className="minigame" role="dialog" aria-label="どっちの手？">
      {report ? (
        <>
          <p className="result">{report.win ? "あたり！🎉" : "ざんねん…"}</p>
          <p>{SIDE_NAMES[report.answer]}の手でした</p>
          <div className="game-actions">
            <button type="button" onClick={() => setReport(null)}>
              もういちど
            </button>
            <button type="button" onClick={onClose}>
              とじる
            </button>
          </div>
        </>
      ) : (
        <>
          <p>どっちの手に入ってるかな？</p>
          <div className="game-actions">
            <button type="button" onClick={() => choose("left")}>
              左の手
            </button>
            <button type="button" onClick={() => choose("right")}>
              右の手
            </button>
          </div>
          <button type="button" className="quit" onClick={onClose}>
            やめる
          </button>
        </>
      )}
    </div>
  );
}
