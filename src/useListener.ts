import { useEffect } from "react";

/**
 * Tauri のイベント購読を、アンマウント時（購読の確立前を含む）に確実に解除する。
 * `subscribe` は購読を始めて解除関数を返す Promise を返す関数。
 */
export function useListener<T>(subscribe: (handler: (value: T) => void) => Promise<() => void>, handler: (value: T) => void) {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    subscribe(handler).then((off) => {
      if (disposed) return off();
      unlisten = off;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
    // 購読は 1 回だけ。handler / subscribe は安定した関数（setState・モジュール関数）を渡す。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
