use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkConstraints {
    Allow,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FsConstraints {
    WriteCwdReadAll { cwd: PathBuf },
    ReadCwdOnly { cwd: PathBuf },
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub cwd: PathBuf,
    pub fs: FsConstraints,
    pub network: NetworkConstraints,
    pub allow_process_fork: bool,
}

impl SandboxPolicy {
    pub fn cwd_writes_only(cwd: PathBuf) -> Self {
        Self {
            fs: FsConstraints::WriteCwdReadAll { cwd: cwd.clone() },
            network: NetworkConstraints::Block,
            cwd,
            allow_process_fork: true,
        }
    }

    pub fn read_only(cwd: PathBuf) -> Self {
        Self {
            fs: FsConstraints::ReadCwdOnly { cwd: cwd.clone() },
            network: NetworkConstraints::Block,
            cwd,
            allow_process_fork: false,
        }
    }

    pub fn policy_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(format!("{:?}", self).as_bytes());
        hex::encode(h.finalize())
    }

    pub fn summary(&self) -> String {
        match &self.fs {
            FsConstraints::WriteCwdReadAll { cwd } => {
                format!("Seatbelt: writes only in {}", cwd.display())
            }
            FsConstraints::ReadCwdOnly { cwd } => {
                format!("Seatbelt: read-only in {}", cwd.display())
            }
            FsConstraints::None => "Seatbelt: no fs access".into(),
        }
    }
}
