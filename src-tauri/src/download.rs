//! Parallel, retrying, hash-verified downloader with a content-addressed cache.
//!
//! - Files are written to `<path>.part`, hash-checked, then renamed into place,
//!   so a half-finished download can never be mistaken for a good file.
//! - When a SHA-1 is known and `cache` is set, the bytes are stored once under
//!   `cache/<aa>/<sha1>` and hard-linked (or copied) into each instance, so the
//!   same mod jar is never downloaded twice.
//! - Existing files are accepted if their size matches (fast path at launch);
//!   `verify_existing` forces a full hash check (repair).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context};
use futures::StreamExt;
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

use crate::core::{emit_progress, sha1_file, sha512_file, Paths};

#[derive(Clone, Debug)]
pub struct DlItem {
    pub urls: Vec<String>,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
    pub cache: bool,
}

impl DlItem {
    pub fn new(url: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        DlItem { urls: vec![url.into()], path: path.into(), sha1: None, sha512: None, size: None, cache: false }
    }
    pub fn sha1(mut self, s: Option<String>) -> Self {
        self.sha1 = s.map(|s| s.to_lowercase());
        self
    }
    pub fn sha512(mut self, s: Option<String>) -> Self {
        self.sha512 = s.map(|s| s.to_lowercase());
        self
    }
    pub fn size(mut self, s: Option<u64>) -> Self {
        self.size = s;
        self
    }
    pub fn cached(mut self) -> Self {
        self.cache = true;
        self
    }
}

pub struct Downloader<'a> {
    pub http: &'a reqwest::Client,
    pub paths: &'a Paths,
    pub app: &'a AppHandle,
    pub threads: usize,
    pub verify_existing: bool,
    /// No progress events (Instant Play pre-loads run invisibly).
    pub silent: bool,
}

impl Downloader<'_> {
    /// Downloads every item; reports progress under `task`. Fails with the
    /// first unrecoverable error after all other downloads finish.
    pub async fn run(&self, task: &str, label: &str, items: Vec<DlItem>) -> anyhow::Result<()> {
        // Dedupe by target path.
        let mut seen = std::collections::HashSet::new();
        let items: Vec<DlItem> = items.into_iter().filter(|i| seen.insert(i.path.clone())).collect();
        let total = items.len() as u64;
        if total == 0 {
            return Ok(());
        }
        let done = Arc::new(AtomicU64::new(0));
        if !self.silent {
            emit_progress(self.app, task, label, 0, total);
        }

        let results: Vec<anyhow::Result<()>> = futures::stream::iter(items)
            .map(|item| {
                let done = done.clone();
                async move {
                    let r = self.fetch_one(&item).await.with_context(|| format!("downloading {}", item.urls[0]));
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if !self.silent && (n == total || n % 8 == 0) {
                        emit_progress(self.app, task, label, n, total);
                    }
                    r
                }
            })
            .buffer_unordered(self.threads.max(1))
            .collect()
            .await;

        let errors: Vec<String> = results.into_iter().filter_map(|r| r.err()).map(|e| format!("{e:#}")).collect();
        if !errors.is_empty() {
            bail!("{} of {} downloads failed. First error: {}", errors.len(), total, errors[0]);
        }
        Ok(())
    }

    async fn fetch_one(&self, item: &DlItem) -> anyhow::Result<()> {
        if self.is_valid(item, &item.path, self.verify_existing).await {
            return Ok(());
        }
        if let Some(parent) = item.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Content-addressed cache hit?
        let cache_path = match (&item.sha1, item.cache) {
            (Some(sha1), true) if sha1.len() >= 2 => Some(self.paths.cache().join(&sha1[..2]).join(sha1)),
            _ => None,
        };
        if let Some(cp) = &cache_path {
            if self.is_valid(item, cp, false).await {
                link_or_copy(cp, &item.path).await?;
                return Ok(());
            }
        }

        let part = item.path.with_extension(format!(
            "{}part",
            item.path.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
        ));
        let mut last_err = None;
        for attempt in 0..4u32 {
            for url in &item.urls {
                match self.stream_to(url, &part).await {
                    Ok(()) => match self.check_hashes(item, &part).await {
                        Ok(()) => {
                            tokio::fs::rename(&part, &item.path).await?;
                            if let Some(cp) = &cache_path {
                                if let Some(p) = cp.parent() {
                                    let _ = tokio::fs::create_dir_all(p).await;
                                }
                                let _ = link_or_copy(&item.path, cp).await;
                            }
                            return Ok(());
                        }
                        Err(e) => last_err = Some(e),
                    },
                    Err(e) => last_err = Some(e),
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500 * 2u64.pow(attempt))).await;
        }
        let _ = tokio::fs::remove_file(&part).await;
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no download URL")))
    }

    async fn stream_to(&self, url: &str, dest: &Path) -> anyhow::Result<()> {
        let resp = self.http.get(url).send().await?.error_for_status()?;
        let mut file = tokio::fs::File::create(dest).await?;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }
        file.flush().await?;
        Ok(())
    }

    async fn check_hashes(&self, item: &DlItem, path: &Path) -> anyhow::Result<()> {
        if let Some(size) = item.size {
            let actual = tokio::fs::metadata(path).await?.len();
            if actual != size {
                bail!("size mismatch: expected {size}, got {actual}");
            }
        }
        if let Some(expected) = &item.sha512 {
            let actual = sha512_file(path).await?;
            if &actual != expected {
                bail!("SHA-512 mismatch");
            }
        } else if let Some(expected) = &item.sha1 {
            let actual = sha1_file(path).await?;
            if &actual != expected {
                bail!("SHA-1 mismatch");
            }
        }
        Ok(())
    }

    async fn is_valid(&self, item: &DlItem, path: &Path, full: bool) -> bool {
        let Ok(meta) = tokio::fs::metadata(path).await else { return false };
        if !meta.is_file() {
            return false;
        }
        if let Some(size) = item.size {
            if meta.len() != size {
                return false;
            }
        }
        // Without a known size we can't fast-path: hash it if we have a hash.
        if full || item.size.is_none() {
            return self.check_hashes(item, path).await.is_ok();
        }
        true
    }
}

async fn link_or_copy(from: &Path, to: &Path) -> anyhow::Result<()> {
    let _ = tokio::fs::remove_file(to).await;
    if tokio::fs::hard_link(from, to).await.is_err() {
        tokio::fs::copy(from, to).await?;
    }
    Ok(())
}

/// Maven coordinate `group:artifact:version[:classifier][@ext]` → relative path.
pub fn maven_path(name: &str) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0].replace('.', "/"), parts[1], parts[2]);
    let file = match parts.get(3) {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("net.fabricmc:fabric-loader:0.16.0").unwrap(), "net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar");
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(maven_path("de.oceanlabs.mcp:mcp_config:1.20.1@zip").unwrap(), "de/oceanlabs/mcp/mcp_config/1.20.1/mcp_config-1.20.1.zip");
        assert!(maven_path("bad").is_none());
    }
}
