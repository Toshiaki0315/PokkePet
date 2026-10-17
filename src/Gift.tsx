import type { GiftItem } from "./api";

/** おでかけから戻ったペットが、おみやげを見せる画面。 */
export function Gift({ items, onThanks }: { items: GiftItem[]; onThanks: () => void }) {
  return (
    <div className="farewell gift" role="dialog" aria-label="おみやげ">
      <p className="title">おでかけから もどったよ！</p>
      <ul className="gift-items">
        {items.map((item, i) => (
          <li key={`${item.name}-${i}`} className={`rarity-${item.rarity}`}>
            <span className="emoji" aria-hidden="true">
              {item.emoji}
            </span>
            <span>{item.name}</span>
          </li>
        ))}
      </ul>
      <p>おみやげを もってきたよ</p>
      <div className="game-actions">
        <button type="button" onClick={onThanks}>
          ありがとう！
        </button>
      </div>
    </div>
  );
}
