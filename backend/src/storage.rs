use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::{fs, io::AsyncWriteExt};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub relative_path: String,
    pub size_bytes: i64,
    pub sha256: String,
}

impl ArtifactStore {
    pub async fn new(root: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&root).await?;
        Ok(Self { root })
    }

    pub async fn put(
        &self,
        floorplan_id: Uuid,
        filename: &str,
        bytes: &[u8],
    ) -> anyhow::Result<StoredArtifact> {
        let safe_name = safe_filename(filename);
        let dir = self.root.join(floorplan_id.to_string());
        fs::create_dir_all(&dir).await?;
        let full_path = dir.join(&safe_name);
        let mut file = fs::File::create(&full_path).await?;
        file.write_all(bytes).await?;
        file.flush().await?;

        Ok(StoredArtifact {
            relative_path: format!("{floorplan_id}/{safe_name}"),
            size_bytes: bytes.len() as i64,
            sha256: sha256_hex(bytes),
        })
    }

    pub async fn read(&self, relative_path: &str) -> anyhow::Result<Vec<u8>> {
        let full_path = self.resolve(relative_path)?;
        Ok(fs::read(full_path).await?)
    }

    fn resolve(&self, relative_path: &str) -> anyhow::Result<PathBuf> {
        if relative_path.contains("..") || Path::new(relative_path).is_absolute() {
            anyhow::bail!("invalid artifact path");
        }
        Ok(self.root.join(relative_path))
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn safe_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_filename_replaces_path_chars() {
        assert_eq!(safe_filename("../hello world.glb"), ".._hello_world.glb");
    }
}
