import { useEffect, useState } from "react";
import { getEggCode, receiveEgg } from "./api";
import "./EggCode.css";

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** メニューバーの「たまごコード…」から開く、友だちとたまごを交換する画面。 */
export default function EggCode() {
  const [mine, setMine] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [friends, setFriends] = useState("");
  const [asking, setAsking] = useState(false);
  const [welcomed, setWelcomed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getEggCode().then(setMine, (e) => setError(errorText(e)));
  }, []);

  const copy = () => {
    if (!mine) return;
    navigator.clipboard.writeText(mine).then(
      () => setCopied(true),
      () => setError("コピーできませんでした。コードを選んでコピーしてください"),
    );
  };

  const welcome = () =>
    receiveEgg(friends).then(
      () => {
        setError(null);
        setAsking(false);
        setWelcomed(true);
        setFriends("");
        getEggCode().then(setMine, (e) => setError(errorText(e)));
      },
      (e) => {
        setAsking(false);
        setError(errorText(e));
      },
    );

  return (
    <main className="eggcode">
      <h1>たまごコード</h1>
      <p className="lead">いまのペットの血筋（性格・色あい・かざり）を、短いコードにして、友だちに送れます。もらったコードのたまごも迎えられます。</p>
      {error && <p role="alert">{error}</p>}

      <section aria-label="わたしのたまごコード">
        <h2>わたしのコード</h2>
        <div className="code-row">
          <output className="code" aria-label="わたしのコード">
            {mine ?? "…"}
          </output>
          <button type="button" onClick={copy} disabled={!mine}>
            {copied ? "コピーしました" : "コピー"}
          </button>
        </div>
      </section>

      <section aria-label="もらったたまご">
        <h2>友だちのたまごを迎える</h2>
        <div className="code-row">
          <input aria-label="友だちのコード" value={friends} placeholder="ABCDE-FGHIJ" maxLength={14} onChange={(e) => { setFriends(e.target.value); setWelcomed(false); }} />
          <button type="button" disabled={friends.trim() === ""} onClick={() => setAsking(true)}>
            迎える
          </button>
        </div>
        {asking && (
          <div className="confirm" role="alertdialog" aria-label="確認">
            <p>いまのペットとはお別れして、友だちのたまごを迎えます。よろしいですか？</p>
            <button type="button" onClick={welcome}>
              迎える
            </button>
            <button type="button" onClick={() => setAsking(false)}>
              やめる
            </button>
          </div>
        )}
        {welcomed && <p className="done">新しいたまごが届きました！ポップアップを開いて、撫でてあげてね。</p>}
      </section>
    </main>
  );
}
