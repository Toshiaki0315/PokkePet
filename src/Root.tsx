import App from "./App";
import Settings from "./Settings";

/** ウィンドウのラベルで表示する画面を切り替える（設定ダイアログ以外はポップアップ）。 */
export default function Root({ label }: { label: string }) {
  return label === "settings" ? <Settings /> : <App />;
}
