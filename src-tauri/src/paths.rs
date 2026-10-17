//! 保存先フォルダの決め方。開発版（`make dev`）とインストールしたアプリがセーブを取り合わないようにする。

use std::path::{Path, PathBuf};

/// 開発版が使うサブフォルダ名。
pub const DEV_DIR: &str = "dev";

/// ゲームのセーブと設定を置くフォルダ。リリース版は `base` そのもの、開発版は `base/dev`。
pub fn data_dir(base: &Path, debug_build: bool) -> PathBuf {
    if debug_build {
        return base.join(DEV_DIR);
    }
    base.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_builds_use_the_standard_app_data_folder() {
        let base = Path::new("/Users/me/Library/Application Support/com.toshiaki.pokkepet");
        assert_eq!(data_dir(base, false), base);
    }

    #[test]
    fn dev_builds_get_their_own_subfolder() {
        let base = Path::new("/Users/me/Library/Application Support/com.toshiaki.pokkepet");
        assert_eq!(data_dir(base, true), base.join("dev"));
    }

    #[test]
    fn the_two_folders_never_overlap() {
        let base = Path::new("/data/app");
        let dev = data_dir(base, true);
        let release = data_dir(base, false);
        assert_ne!(dev, release);
        assert!(dev.starts_with(&release)); // 開発版は中のサブフォルダ（リリース版のファイルとは別の場所）
        assert_ne!(dev.join("save.json"), release.join("save.json"));
        assert_ne!(dev.join("settings.json"), release.join("settings.json"));
    }
}
