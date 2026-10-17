import type { Branch, Personality } from "./api";

// 図鑑・思い出で使う、分岐と性格（種族）の表示名。
export const BRANCH_NAMES: Record<Branch, string> = { poor: "しょんぼり", normal: "ふつう", good: "きらきら", legend: "でんせつ" };
export const SPECIES_NAMES: Record<Personality, string> = { calm: "ふわもち", playful: "きつね", gluttonous: "ぽっちゃり", tidy: "うさぎ" };

/** 生活リズムから学んだ朝型・夜型の表示名。 */
export const CHRONOTYPE_NAMES: Record<"morning" | "night" | "neutral", string> = { morning: "朝型", night: "夜型", neutral: "ふつう型" };
