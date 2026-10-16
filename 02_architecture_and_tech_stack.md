# アーキテクチャ＆技術スタック設計

## 1. 技術スタック
- **バックエンド (コアロジック＆状態管理)**: Rust (Tauri v2)
- **フロントエンド (UI描画)**: TypeScript, React, Vite
- **通信**: Tauri IPC (Inter-Process Communication)

## 2. 役割分担
### Rust (バックエンド)
- ゲームループの実行（バックグラウンドでの時間経過、パラメータ自動変動処理）。
- SQLite またはローカルJSONファイルへのセーブデータの永続化。
- ステータス計算、進化条件の判定ロジック。
- macOSメニューバー（トレイ）アイコンの制御。

### TypeScript (フロントエンド)
- ドット絵アニメーションのレンダリング。
- ユーザーアクション（ご飯、遊ぶ、掃除）の入力受付とIPC経由でのRust呼び出し。
- 最新ステータスのポーリングまたはイベント受信による画面更新。

## 3. ソースコード管理
- **ホスティング**: GitHub
- **リポジトリURL**: https://github.com/Toshiaki0315/PokkePet.git
- **ブランチ戦略**: GitHub Flow (mainブランチ + featureブランチ)