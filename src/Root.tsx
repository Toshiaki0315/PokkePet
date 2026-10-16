import App from "./App";
import Dex from "./Dex";
import Settings from "./Settings";

/** ウィンドウのラベルで表示する画面を切り替える（設定・図鑑以外はポップアップ）。 */
export default function Root({ label }: { label: string }) {
  if (label === "settings") return <Settings />;
  if (label === "dex") return <Dex />;
  return <App />;
}
