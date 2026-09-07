use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid path")]
    InvalidPath,
    #[error("Not found")]
    NotFound,
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
    async fn write(&self, user_id: &str, relative_path: &str, content: &[u8]) -> Result<()>;
    async fn read(&self, user_id: &str, relative_path: &str) -> Result<Vec<u8>>;
    async fn delete(&self, user_id: &str, relative_path: &str) -> Result<()>;
    async fn rename(&self, user_id: &str, old_path: &str, new_path: &str) -> Result<()>;
    async fn copy(&self, user_id: &str, src_path: &str, dst_path: &str) -> Result<()>;
    async fn exists(&self, user_id: &str, relative_path: &str) -> Result<bool>;
}

pub fn normalize_relative_path(path: &str) -> Result<String> {
    let path = path.replace('\\', "/");
    let parts: Vec<&str> = path.split('/').collect();
    let mut normalized = Vec::new();

    for part in parts {
        match part {
            "" | "." => continue,
            ".." => {
                if normalized.is_empty() {
                    return Err(StorageError::InvalidPath);
                }
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }

    if normalized.is_empty() {
        return Ok(".".to_string());
    }

    Ok(normalized.join("/"))
}

pub fn user_files_dir(storage_root: &Path, user_id: &str) -> PathBuf {
    storage_root.join("users").join(user_id).join("files")
}

pub fn user_conflicts_dir(storage_root: &Path, user_id: &str) -> PathBuf {
    storage_root.join("users").join(user_id).join("conflicts")
}

pub struct LocalDiskStorageProvider {
    storage_root: PathBuf,
}

impl LocalDiskStorageProvider {
    pub fn new(storage_root: impl Into<PathBuf>) -> Self {
        Self {
            storage_root: storage_root.into(),
        }
    }

    pub fn init(&self) -> Result<()> {
        let base = &self.storage_root;
        std::fs::create_dir_all(base.join("users"))?;
        Ok(())
    }

    fn full_path(&self, user_id: &str, relative_path: &str) -> Result<PathBuf> {
        let normalized = normalize_relative_path(relative_path)?;
        if normalized == "." {
            return Err(StorageError::InvalidPath);
        }
        Ok(user_files_dir(&self.storage_root, user_id).join(&normalized))
    }
}

#[async_trait::async_trait]
impl StorageProvider for LocalDiskStorageProvider {
    async fn write(&self, user_id: &str, relative_path: &str, content: &[u8]) -> Result<()> {
        let path = self.full_path(user_id, relative_path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    async fn read(&self, user_id: &str, relative_path: &str) -> Result<Vec<u8>> {
        let path = self.full_path(user_id, relative_path)?;
        if !path.exists() {
            return Err(StorageError::NotFound);
        }
        Ok(std::fs::read(path)?)
    }

    async fn delete(&self, user_id: &str, relative_path: &str) -> Result<()> {
        let path = self.full_path(user_id, relative_path)?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    async fn rename(&self, user_id: &str, old_path: &str, new_path: &str) -> Result<()> {
        let src = self.full_path(user_id, old_path)?;
        let dst = self.full_path(user_id, new_path)?;
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(src, dst)?;
        Ok(())
    }

    async fn copy(&self, user_id: &str, src_path: &str, dst_path: &str) -> Result<()> {
        let src = self.full_path(user_id, src_path)?;
        let dst = self.full_path(user_id, dst_path)?;
        if !src.exists() {
            return Err(StorageError::NotFound);
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
        Ok(())
    }

    async fn exists(&self, user_id: &str, relative_path: &str) -> Result<bool> {
        let path = self.full_path(user_id, relative_path)?;
        Ok(path.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn storage_provider_crud() {
        let dir = tempdir().unwrap();
        let provider = Arc::new(LocalDiskStorageProvider::new(dir.path()));
        provider.init().unwrap();

        let user_id = "user-test";
        let path = "docs/readme.txt";
        let content = b"Hola SyncFiles";

        assert!(!provider.exists(user_id, path).await.unwrap());

        provider.write(user_id, path, content).await.unwrap();
        assert!(provider.exists(user_id, path).await.unwrap());

        let read = provider.read(user_id, path).await.unwrap();
        assert_eq!(read, content);

        provider.rename(user_id, path, "docs/readme-v2.txt").await.unwrap();
        assert!(!provider.exists(user_id, path).await.unwrap());
        assert!(provider.exists(user_id, "docs/readme-v2.txt").await.unwrap());

        provider.copy(user_id, "docs/readme-v2.txt", "docs/readme-copy.txt").await.unwrap();
        assert!(provider.exists(user_id, "docs/readme-copy.txt").await.unwrap());

        provider.delete(user_id, "docs/readme-v2.txt").await.unwrap();
        assert!(!provider.exists(user_id, "docs/readme-v2.txt").await.unwrap());
        assert!(provider.exists(user_id, "docs/readme-copy.txt").await.unwrap());
    }

    #[tokio::test]
    async fn storage_provider_rejects_invalid_paths() {
        let dir = tempdir().unwrap();
        let provider = Arc::new(LocalDiskStorageProvider::new(dir.path()));
        provider.init().unwrap();

        let user_id = "user-test";
        let err = provider.write(user_id, "../etc/passwd", b"x").await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn storage_provider_normalizes_paths() {
        let dir = tempdir().unwrap();
        let provider = Arc::new(LocalDiskStorageProvider::new(dir.path()));
        provider.init().unwrap();

        let user_id = "user-test";
        provider.write(user_id, "a/b/../c/file.txt", b"data").await.unwrap();
        assert!(provider.exists(user_id, "a/c/file.txt").await.unwrap());
    }
}
