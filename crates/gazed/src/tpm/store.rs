// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sealed-blob persistence, deliberately independent of TPM hardware and ESAPI.

use anyhow::{Context as _, anyhow, ensure};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub(super) const PUB_FILE: &str = "dek.pub";
pub(super) const PRIV_FILE: &str = "dek.priv";
pub(super) const BUNDLE_FILE: &str = "dek.blob";
const BUNDLE_MAGIC: &[u8; 5] = b"GZDK1";

fn read_private_file(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let meta = file.metadata()?;
    ensure!(
        meta.is_file()
            && meta.uid() == unsafe { libc::geteuid() }
            && meta.mode() & 0o077 == 0
            && meta.nlink() == 1,
        "密封密钥文件不安全：{}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16384, "密封密钥文件过大");
    Ok(Some(bytes))
}

pub(super) fn read_sealed_blobs(dir: &Path) -> anyhow::Result<Option<(Vec<u8>, Vec<u8>)>> {
    if let Some(bytes) = read_private_file(&dir.join(BUNDLE_FILE))? {
        ensure!(
            bytes.len() >= 9 && bytes.starts_with(BUNDLE_MAGIC),
            "密封密钥包无效"
        );
        let public_len = u32::from_le_bytes(bytes[5..9].try_into()?) as usize;
        let private_len = bytes.len().saturating_sub(9 + public_len);
        ensure!(
            (1..=4096).contains(&public_len) && (1..=4096).contains(&private_len),
            "密封密钥包长度无效"
        );
        let public = &bytes[9..9 + public_len];
        let private = &bytes[9 + public_len..];
        verify_legacy_blobs(dir, public, private)?;
        return Ok(Some((public.to_vec(), private.to_vec())));
    }
    match (
        read_private_file(&dir.join(PUB_FILE))?,
        read_private_file(&dir.join(PRIV_FILE))?,
    ) {
        (None, None) => Ok(None),
        (Some(public), Some(private)) => Ok(Some((public, private))),
        _ => Err(anyhow!(
            "旧版 TPM 密钥对不完整；请从备份恢复缺失的数据；拒绝生成替代密钥"
        )),
    }
}

fn verify_legacy_blobs(dir: &Path, public: &[u8], private: &[u8]) -> anyhow::Result<()> {
    for (name, expected) in [(PUB_FILE, public), (PRIV_FILE, private)] {
        if let Some(bytes) = read_private_file(&dir.join(name))? {
            ensure!(
                bytes == expected,
                "TPM 密钥文件冲突：{} 与密封密钥包不匹配；请保留所有密钥文件，并从备份恢复匹配的密钥组",
                dir.join(name).display()
            );
        }
    }
    Ok(())
}

pub(super) fn persist_bundle(dir: &Path, public: &[u8], private: &[u8]) -> anyhow::Result<()> {
    ensure!(
        (1..=4096).contains(&public.len()) && (1..=4096).contains(&private.len()),
        "密封密钥长度无效"
    );
    verify_legacy_blobs(dir, public, private)?;
    let mut bytes = BUNDLE_MAGIC.to_vec();
    bytes.extend_from_slice(&(public.len() as u32).to_le_bytes());
    bytes.extend_from_slice(public);
    bytes.extend_from_slice(private);
    match read_private_file(&dir.join(BUNDLE_FILE))? {
        Some(existing) => ensure!(existing == bytes, "密封密钥包已改变；拒绝覆盖"),
        None => write_private_file(&dir.join(BUNDLE_FILE), &bytes)?,
    }
    // Sync even an existing bundle before removing either legacy blob: it may have
    // survived a previous rename whose directory sync failed.
    File::open(dir)?.sync_all()?;
    for name in [PUB_FILE, PRIV_FILE] {
        match fs::remove_file(dir.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    File::open(dir)?.sync_all()?;
    Ok(())
}

pub(super) fn ensure_private_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(std::io::Error::other(format!(
            "{} 不是私有目录",
            path.display()
        )));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    File::open(path)?.sync_all()?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

pub(super) fn write_private_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("密封密钥路径没有父目录")?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("dek"),
        uuid::Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("无法创建 {}", tmp.display()))?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&tmp);
        return Err(error).with_context(|| format!("无法写入 {}", tmp.display()));
    }
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error).with_context(|| format!("无法替换 {}", path.display()));
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn bundle_round_trip_and_interrupted_legacy_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        ensure_private_dir(dir.path()).unwrap();
        write_private_file(&dir.path().join(PUB_FILE), b"public").unwrap();
        assert!(read_sealed_blobs(dir.path()).is_err());
        write_private_file(&dir.path().join(PRIV_FILE), b"private").unwrap();
        persist_bundle(dir.path(), b"public", b"private").unwrap();
        for (name, bytes) in [(PUB_FILE, b"public".as_slice()), (PRIV_FILE, b"private")] {
            write_private_file(&dir.path().join(name), bytes).unwrap();
            assert_eq!(
                read_sealed_blobs(dir.path()).unwrap(),
                Some((b"public".to_vec(), b"private".to_vec()))
            );
            persist_bundle(dir.path(), b"public", b"private").unwrap();
            assert!(!dir.path().join(name).exists());
        }
        assert_eq!(
            fs::metadata(dir.path().join(BUNDLE_FILE)).unwrap().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn conflicting_key_files_are_preserved() {
        for (public, private) in [
            (
                Some(b"old public".as_slice()),
                Some(b"old private".as_slice()),
            ),
            (Some(b"old public".as_slice()), Some(b"private".as_slice())),
            (Some(b"public".as_slice()), Some(b"old private".as_slice())),
            (Some(b"old public".as_slice()), None),
            (None, Some(b"old private".as_slice())),
        ] {
            let dir = tempfile::tempdir().unwrap();
            persist_bundle(dir.path(), b"public", b"private").unwrap();
            for (name, bytes) in [(PUB_FILE, public), (PRIV_FILE, private)] {
                if let Some(bytes) = bytes {
                    write_private_file(&dir.path().join(name), bytes).unwrap();
                }
            }
            let snapshot = || {
                [BUNDLE_FILE, PUB_FILE, PRIV_FILE].map(|name| fs::read(dir.path().join(name)).ok())
            };
            let before = snapshot();
            assert!(
                read_sealed_blobs(dir.path())
                    .unwrap_err()
                    .to_string()
                    .contains("TPM 密钥文件冲突")
            );
            assert_eq!(snapshot(), before);
            assert!(persist_bundle(dir.path(), b"public", b"private").is_err());
            assert_eq!(snapshot(), before);
        }
    }

    #[test]
    fn matching_legacy_pair_can_be_cleaned_up() {
        let dir = tempfile::tempdir().unwrap();
        persist_bundle(dir.path(), b"public", b"private").unwrap();
        write_private_file(&dir.path().join(PUB_FILE), b"public").unwrap();
        write_private_file(&dir.path().join(PRIV_FILE), b"private").unwrap();
        assert_eq!(
            read_sealed_blobs(dir.path()).unwrap(),
            Some((b"public".to_vec(), b"private".to_vec()))
        );
        persist_bundle(dir.path(), b"public", b"private").unwrap();
        assert!(!dir.path().join(PUB_FILE).exists());
        assert!(!dir.path().join(PRIV_FILE).exists());
    }

    #[test]
    fn conflicting_legacy_pair_prevents_bundle_creation() {
        let dir = tempfile::tempdir().unwrap();
        write_private_file(&dir.path().join(PUB_FILE), b"old public").unwrap();
        write_private_file(&dir.path().join(PRIV_FILE), b"old private").unwrap();
        assert!(persist_bundle(dir.path(), b"public", b"private").is_err());
        assert!(!dir.path().join(BUNDLE_FILE).exists());
        assert_eq!(fs::read(dir.path().join(PUB_FILE)).unwrap(), b"old public");
        assert_eq!(
            fs::read(dir.path().join(PRIV_FILE)).unwrap(),
            b"old private"
        );
    }

    #[test]
    fn corrupt_or_replaced_bundles_never_fall_back_to_legacy_keys() {
        let dir = tempfile::tempdir().unwrap();
        write_private_file(&dir.path().join(PUB_FILE), b"public").unwrap();
        write_private_file(&dir.path().join(PRIV_FILE), b"private").unwrap();
        for bytes in [
            b"GZDK1".as_slice(),
            b"GZDK1\xff\xff\xff\xff",
            b"GZDK1\0\0\0\0private",
        ] {
            write_private_file(&dir.path().join(BUNDLE_FILE), bytes).unwrap();
            assert!(read_sealed_blobs(dir.path()).is_err());
            assert!(persist_bundle(dir.path(), b"public", b"private").is_err());
            assert!(dir.path().join(PUB_FILE).exists());
            assert!(dir.path().join(PRIV_FILE).exists());
        }
    }

    #[test]
    fn unsafe_blob_types_and_permissions_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(BUNDLE_FILE);
        let target = dir.path().join("target");
        write_private_file(&target, b"bundle").unwrap();
        symlink(&target, &path).unwrap();
        assert!(read_sealed_blobs(dir.path()).is_err());
        fs::remove_file(&path).unwrap();
        fs::hard_link(&target, &path).unwrap();
        assert!(read_sealed_blobs(dir.path()).is_err());
        fs::remove_file(&path).unwrap();
        write_private_file(&path, b"bundle").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_sealed_blobs(dir.path()).is_err());
    }
}
