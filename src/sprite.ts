import type { StateView } from "./api";

export function spritePath({ stage, branch, personality }: Pick<StateView, "stage" | "branch" | "personality">): string {
  if (stage <= 1) return "/sprites/egg.png";
  return `/sprites/${stage}-${branch}-${personality}.png`;
}
