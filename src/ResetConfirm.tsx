/** 隠しコマンドで出る、「卵からやり直す」の確認。うっかり押しても、ここで止められる。 */
export function ResetConfirm({ name, onConfirm, onCancel }: { name: string | null; onConfirm: () => void; onCancel: () => void }) {
  return (
    <div className="farewell reset-confirm" role="dialog" aria-label="やり直し">
      <p className="title">卵からやり直しますか？</p>
      <p>{name ? `${name}とはお別れになります。` : "今のペットとはお別れになります。"}</p>
      <p>図鑑と思い出は残ります。</p>
      <div className="game-actions">
        <button type="button" onClick={onConfirm}>
          やり直す
        </button>
        <button type="button" onClick={onCancel}>
          やめる
        </button>
      </div>
    </div>
  );
}
