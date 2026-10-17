import App from "./App";
import Dex from "./Dex";
import Diary from "./Diary";
import Settings from "./Settings";
import Souvenirs from "./Souvenirs";

/** ウィンドウのラベルで表示する画面を切り替える（設定・図鑑・思い出以外はポップアップ）。 */
export default function Root({ label }: { label: string }) {
  if (label === "settings") return <Settings />;
  if (label === "dex") return <Dex />;
  if (label === "diary") return <Diary />;
  if (label === "souvenirs") return <Souvenirs />;
  return <App />;
}
