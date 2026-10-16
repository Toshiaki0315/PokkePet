# 環境構築ガイド (Apple Silicon macOS)

## 1. 必須ツールのインストール
以下のツールがインストールされていることを確認（または Homebrew 経由でインストール）してください。
- Node.js (v20以上) または pnpm
- Rust (`rustup` 経由)

## 2. Rust ターゲットの追加
Apple Silicon専用ビルドのため、ターゲットを追加します。
```bash
rustup target add aarch64-apple-darwin
```

## 3. プロジェクトの初期化
Tauri CLIを使用してプロジェクトの雛形を作成します。（Claude実行用ステップ）
```bash
npx create-tauri-app@latest PokkePet
# または
pnpm create tauri-app PokkePet
# [選択] TypeScript / React / Vite
```

## 4. テスト環境のセットアップ
- **Rust**: カバレッジ計測用に `cargo-tarpaulin` をインストール。
  `cargo install cargo-tarpaulin`
- **TypeScript**: `vitest` および `@vitest/coverage-v8` をインストール。