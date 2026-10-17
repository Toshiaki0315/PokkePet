/** 亡くなったとき（きびしいモード）のお墓の画面。見送ると新しい卵が届く。 */
export function Grave({ name, onBury }: { name: string | null; onBury: () => void }) {
  return (
    <div className="farewell grave" role="dialog" aria-label="お別れ">
      <p className="title" aria-hidden="true">
        🪦
      </p>
      <p>{name ? `${name}は天国へ旅立ちました…` : "天国へ旅立ちました…"}</p>
      <p>いっしょに過ごした日々は「思い出」に残っています。</p>
      <div className="game-actions">
        <button type="button" onClick={onBury}>
          新しい卵を迎える
        </button>
      </div>
    </div>
  );
}
