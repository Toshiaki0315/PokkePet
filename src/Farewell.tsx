/** 旅立ちの時のお別れ画面。見送ると次の世代の卵が届く。 */
export function Farewell({ name, onSeeOff, onStay }: { name: string | null; onSeeOff: () => void; onStay: () => void }) {
  return (
    <div className="farewell" role="dialog" aria-label="旅立ち">
      <p className="title">旅立ちの時がきたよ</p>
      <p>{name ? `${name}はりっぱに育ちました。ありがとう！` : "りっぱに育ちました。ありがとう！"}</p>
      <div className="game-actions">
        <button type="button" onClick={onSeeOff}>
          見送る
        </button>
        <button type="button" onClick={onStay}>
          もう少しいっしょにいる
        </button>
      </div>
    </div>
  );
}
