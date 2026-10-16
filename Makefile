# PokkePet 開発用コマンド (Apple Silicon macOS 専用)
TARGET := aarch64-apple-darwin
BUNDLE_DIR := src-tauri/target/$(TARGET)/release/bundle

.PHONY: dev app dmg

## 開発サーバー(トレイ常駐アプリ)を起動する。時間短縮: make dev TIME_SCALE=240
dev:
	$(if $(TIME_SCALE),POKKEPET_TIME_SCALE=$(TIME_SCALE)) pnpm tauri dev

## リリースビルドして .app を作る -> $(BUNDLE_DIR)/macos/PokkePet.app
app:
	pnpm install --frozen-lockfile
	pnpm tauri build --target $(TARGET) --bundles app

## リリースビルドして .dmg を作る -> $(BUNDLE_DIR)/dmg/
dmg:
	pnpm install --frozen-lockfile
	pnpm tauri build --target $(TARGET) --bundles dmg
