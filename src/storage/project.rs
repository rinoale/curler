use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::domain::request::stable_hash;

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub root: PathBuf,
    pub history_dir: PathBuf,
    pub history_file: PathBuf,
    pub state_file: PathBuf,
    pub name: String,
}

impl ProjectContext {
    pub fn discover() -> io::Result<Self> {
        let cwd = env::current_dir()?;
        let root = find_project_root(&cwd);
        let root = fs::canonicalize(&root).unwrap_or(root);
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string();
        let history_dir = histories_root()?.join(project_dir_name(&root, &name));

        fs::create_dir_all(&history_dir)?;

        Ok(Self {
            root,
            history_file: history_dir.join("history.json"),
            state_file: history_dir.join("state.json"),
            history_dir,
            name,
        })
    }
}

fn find_project_root(start: &Path) -> PathBuf {
    let mut current = start;

    loop {
        if has_project_marker(current) {
            return current.to_path_buf();
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => return start.to_path_buf(),
        }
    }
}

fn has_project_marker(path: &Path) -> bool {
    [
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
    ]
    .iter()
    .any(|marker| path.join(marker).exists())
}

fn histories_root() -> io::Result<PathBuf> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;

    Ok(home.join(".curler").join("projects").join("histories"))
}

fn project_dir_name(root: &Path, name: &str) -> String {
    let root = root.to_string_lossy();
    let name = sanitize_name(name);

    format!("{name}-{}", &stable_hash(&[root.as_ref()])[..8])
}

fn sanitize_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "project".to_string()
    } else {
        sanitized
    }
}
