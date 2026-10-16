import { useEffect, useRef } from "react";

/** 隠しコマンド: 直近のキー入力が `sequence` と一致したら `onMatch` を呼ぶ。 */
export function useHiddenCommand(sequence: readonly string[], onMatch: () => void) {
  const callback = useRef(onMatch);
  useEffect(() => {
    callback.current = onMatch;
  });

  useEffect(() => {
    let recent: string[] = [];
    const onKeyDown = (event: KeyboardEvent) => {
      recent = [...recent, event.key].slice(-sequence.length);
      if (recent.join("\0") !== sequence.join("\0")) return;
      recent = [];
      callback.current();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [sequence]);
}
