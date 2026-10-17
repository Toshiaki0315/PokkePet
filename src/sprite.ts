import type { StateView } from "./api";

/** ねむっている（`sleeping`）ときは、目を閉じた絵を使う。卵は目がないので、いつも同じ絵。 */
export function spritePath({ stage, branch, personality, sleeping = false }: Pick<StateView, "stage" | "branch" | "personality"> & { sleeping?: boolean }): string {
  if (stage <= 1) return "/sprites/egg.png";
  return `/sprites/${stage}-${branch}-${personality}${sleeping ? "-sleep" : ""}.png`;
}
