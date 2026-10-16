import { useState } from "react";
import type { FormEvent } from "react";

/** 名前の入力欄の最大文字数。検証の本体は Rust（NAME_MAX_CHARS）で、これは入力しやすくするための目安。 */
export const NAME_INPUT_MAX = 8;

/** 孵化したあとに名前を付けてもらう画面。 */
export function NamePrompt({ onSubmit, onLater }: { onSubmit: (name: string) => void; onLater: () => void }) {
  const [name, setNameText] = useState("");
  const blank = name.trim() === "";
  // 空のときは送信ボタンが無効なので、Enter でも送信されない（検証の本体は Rust）。
  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit(name);
  };
  return (
    <form className="name-prompt" onSubmit={submit}>
      <p>名前をつけてあげてね</p>
      <input aria-label="名前" value={name} maxLength={NAME_INPUT_MAX} onChange={(e) => setNameText(e.target.value)} />
      <div className="name-actions">
        <button type="submit" disabled={blank}>
          決定
        </button>
        <button type="button" onClick={onLater}>
          あとで
        </button>
      </div>
    </form>
  );
}
