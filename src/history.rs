use std::{fs, io, path::Path, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::request::RequestDraft;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectHistory {
    pub project_root: String,
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub origin: String,
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub request: RequestDraft,
    pub created_at: u64,
    pub updated_at: u64,
    pub run_count: u64,
}

impl ProjectHistory {
    pub fn load(path: &Path, project_root: &Path) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self {
                project_root: project_root.to_string_lossy().to_string(),
                entries: Vec::new(),
            });
        }

        let contents = fs::read_to_string(path)?;
        let mut history = serde_json::from_str::<Self>(&contents).map_err(invalid_data)?;

        history.project_root = project_root.to_string_lossy().to_string();

        Ok(history)
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let contents = serde_json::to_string_pretty(self).map_err(invalid_data)?;

        fs::write(path, contents)
    }

    pub fn upsert(&mut self, request: RequestDraft) -> String {
        let id = request.fingerprint();
        let now = unix_timestamp();

        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.updated_at = now;
            entry.run_count += 1;
            entry.request = request;
            return id;
        }

        self.entries.push(HistoryEntry {
            id: id.clone(),
            name: None,
            origin: request.origin.clone(),
            method: request.method.clone(),
            path: request.path.clone(),
            query: request.query.clone(),
            request,
            created_at: now,
            updated_at: now,
            run_count: 1,
        });

        id
    }

    pub fn latest(&self) -> Option<&HistoryEntry> {
        self.entries.iter().max_by_key(|entry| entry.updated_at)
    }

    pub fn delete_entry(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id);
        self.entries.len() != before
    }

    pub fn delete_route(&mut self, origin: &str, method: &str, path: &str) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|entry| entry.origin != origin || entry.method != method || entry.path != path);
        before - self.entries.len()
    }

    pub fn delete_host(&mut self, origin: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.origin != origin);
        before - self.entries.len()
    }

    pub fn rename_entry(&mut self, id: &str, name: Option<String>) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) else {
            return false;
        };

        entry.name = name;
        entry.updated_at = unix_timestamp();

        true
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn invalid_data(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
