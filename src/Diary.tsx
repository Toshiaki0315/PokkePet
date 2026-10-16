import { useCallback, useEffect, useState } from "react";
import { getDiary, onStateChanged } from "./api";
import type { DiaryEvent, DiaryView } from "./api";
import { BRANCH_NAMES, SPECIES_NAMES } from "./names";
import { useListener } from "./useListener";
import "./Diary.css";

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const two = (n: number) => String(n).padStart(2, "0");

/** 「10/17 07:05」（ローカル時刻）。 */
const formatTime = (unix: number) => {
  const d = new Date(unix * 1000);
  return `${d.getMonth() + 1}/${d.getDate()} ${two(d.getHours())}:${two(d.getMinutes())}`;
};

const describe = (event: DiaryEvent): string => {
  switch (event.kind) {
    case "born":
      return "卵が届いた";
    case "hatched":
      return "卵が孵った！";
    case "evolved":
      return `段階 ${event.stage} に進化した（${BRANCH_NAMES[event.branch]}・${SPECIES_NAMES[event.personality]}）`;
    case "named":
      return `「${event.name}」と名づけた`;
    case "careMilestone":
      return `お世話が ${event.count} 回になった`;
    case "graduated":
      return event.name ? `「${event.name}」が旅立った（${SPECIES_NAMES[event.personality]}）` : `旅立ちを見送った（${SPECIES_NAMES[event.personality]}）`;
  }
};

/** メニューバーの「思い出…」から開く、これまでの出来事の記録。 */
export default function Diary() {
  const [diary, setDiary] = useState<DiaryView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getDiary().then(
      (next) => {
        setError(null);
        setDiary(next);
      },
      (e) => setError(errorText(e)),
    );
  }, []);

  useEffect(load, [load]);
  // 進化や名づけで思い出が増えるので、状態が変わるたびに読み直す。
  useListener(onStateChanged, load);

  return (
    <main className="diary">
      <h1>思い出</h1>
      {error && <p role="alert">{error}</p>}
      {diary && (
        <>
          <p className="days">いっしょに {diary.daysTogether} 日目</p>
          {diary.entries.length === 0 ? (
            <p className="empty">まだ思い出がありません</p>
          ) : (
            <ul aria-label="思い出">
              {diary.entries.map((entry, i) => (
                <li key={`${entry.atUnix}-${i}`}>
                  <time>{formatTime(entry.atUnix)}</time>
                  <span>{describe(entry.event)}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </main>
  );
}
