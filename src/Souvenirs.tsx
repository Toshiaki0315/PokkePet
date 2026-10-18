import { useCallback, useEffect, useState } from "react";
import { getSouvenirs, onStateChanged, toggleSouvenir } from "./api";
import type { SouvenirEntry, SouvenirView } from "./api";
import { useListener } from "./useListener";
import "./Souvenirs.css";

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export const RARITY_NAMES = ["よくある", "めずらしい", "とてもめずらしい"] as const;

/** 季節限定のしるし。 */
export const SEASON_MARKS = { spring: "🌸", summer: "🌻", autumn: "🍁", winter: "❄️" } as const;
const SEASON_NAMES = { spring: "春", summer: "夏", autumn: "秋", winter: "冬" } as const;

// 集めていないおみやげは名前を出さない（アクセシビリティ名も「まだ」）。
const label = (e: SouvenirEntry) => (e.count > 0 ? `${e.name}（${RARITY_NAMES[e.rarity]}${e.season ? `・${SEASON_NAMES[e.season]}限定` : ""}）` : e.season ? `まだ見つけていない（${SEASON_NAMES[e.season]}限定）` : "まだ見つけていない");

/** メニューバーの「おみやげ…」から開く、おでかけで持ち帰ったおみやげのコレクション。クリックで画面に飾る／しまう。 */
export default function Souvenirs() {
  const [book, setBook] = useState<SouvenirView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getSouvenirs().then(
      (next) => {
        setError(null);
        setBook(next);
      },
      (e) => setError(errorText(e)),
    );
  }, []);

  useEffect(load, [load]);
  // おでかけから戻るたびにおみやげが増えるので、状態が変わるたびに読み直す。
  useListener(onStateChanged, load);

  const toggle = (id: string) =>
    toggleSouvenir(id).then(
      (next) => {
        setError(null);
        setBook(next);
      },
      (e) => setError(errorText(e)),
    );

  return (
    <main className="souvenirs">
      <h1>おみやげ</h1>
      <p className="lead">席を離れているあいだ、ペットはおでかけして、戻るとおみやげを持ってきます。</p>
      {error && <p role="alert">{error}</p>}
      {book && (
        <>
          <div className="summary">
            <span>
              集めた {book.found} / {book.total}
            </span>
            <progress aria-label="集めた数" role="progressbar" aria-valuenow={book.found} max={book.total} value={book.found} />
          </div>
          <p className="hint">持っているものをクリックすると、画面に飾れます（{book.maxDisplayed} つまで）。🌸🌻🍁❄️ のしるしは、その季節にしか見つからないものです。</p>
          <ul className="grid" aria-label="おみやげ">
            {book.entries.map((e) => (
              <li key={e.id}>
                <button type="button" className={`item rarity-${e.rarity}${e.displayed ? " displayed" : ""}`} disabled={e.count === 0} aria-pressed={e.displayed} aria-label={label(e)} title={label(e)} onClick={() => toggle(e.id)}>
                  <span className="emoji" aria-hidden="true">
                    {e.count > 0 ? e.emoji : "？"}
                  </span>
                  {e.season && (
                    <span className="season" aria-hidden="true">
                      {SEASON_MARKS[e.season]}
                    </span>
                  )}
                  {e.count > 1 && <span className="count">×{e.count}</span>}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </main>
  );
}
