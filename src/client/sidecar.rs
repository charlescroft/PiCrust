use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub struct SidecarProcess {
    child: Option<Child>,
    pub port: u16,
    pub token: String,
    pub instance_id: String,
}

impl SidecarProcess {
    pub fn spawn_if_available(port: u16, token: String, instance_id: String) -> Option<Self> {
        let (sidecar_script, bundle_path, repo_root) = locate_sidecar()?;
        if !bundle_path.exists() {
            return None;
        }

        let mut cmd = Command::new("node");
        cmd.arg(&sidecar_script)
            .current_dir(&repo_root)
            .env("PI_SIDECAR_HOST", "127.0.0.1")
            .env("PI_SIDECAR_PORT", port.to_string())
            .env("PI_DESKTOP_API_TOKEN", &token)
            .env("PI_DESKTOP_INSTANCE_ID", &instance_id)
            .env("PI_SIDECAR_BUNDLE", &bundle_path)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());

        match cmd.spawn() {
            Ok(child) => Some(Self {
                child: Some(child),
                port,
                token,
                instance_id,
            }),
            Err(_) => None,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for SidecarProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn locate_sidecar() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [
        cwd.join("pi-gpui"),
        cwd.clone(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    ];

    for dir in candidates {
        let script = dir.join("sidecar").join("api-sidecar.mjs");
        let bundle = dir.join("dist-sidecar").join("server.cjs");
        if script.exists() {
            let repo_root = dir.parent().map(Path::to_path_buf).unwrap_or(cwd.clone());
            return Some((script, bundle, repo_root));
        }
    }
    None
}
