import { useEffect, useState } from "react";

/**
 * 指定のキー（`withMeta` なら ⌘ と一緒に）を押している間だけ true を返す。
 * macOS は ⌘ を押している間、ほかのキーを離した通知を送らないことがあるので、⌘ を離したときも「離した」とみなす。
 * ウィンドウが非アクティブになったときも離した扱い（押したまま別のウィンドウへ移っても表示が残らないように）。
 */
export function useHeldKey(key: string, withMeta = false): boolean {
  const [held, setHeld] = useState(false);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== key || (withMeta && !event.metaKey)) return;
      event.preventDefault(); // ⌘P の「印刷」などの既定の動作を止める
      setHeld(true);
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === key || event.key === "Meta") setHeld(false);
    };
    const release = () => setHeld(false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", release);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", release);
    };
  }, [key, withMeta]);

  return held;
}
