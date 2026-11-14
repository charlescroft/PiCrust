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

        let node_bin = resolve_node_binary();
        let mut cmd = Command::new(node_bin);
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

fn resolve_node_binary() -> PathBuf {
    let candidates = [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/Applications/Pi Agent.app/Contents/Resources/resources/Pi Agent Server.app/Contents/MacOS/node",
    ];
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(if cfg!(windows) { "node.exe" } else { "node" });
            if p.is_file() {
                return p;
            }
        }
    }
    for c in candidates {
        let p = PathBuf::from(c);
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("node")
}

fn locate_sidecar() -> Option<(PathBuf, PathBuf, PathBuf)> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(contents) = exe.parent().and_then(Path::parent) {
            let res_dir = contents.join("Resources").join("sidecar");
            let script = res_dir.join("api-sidecar.mjs");
            let mjs = res_dir.join("server.mjs");
            let cjs = res_dir.join("server.cjs");
            let bundle = if mjs.exists() { mjs } else { cjs };
            if script.exists() && bundle.exists() {
                return Some((script, bundle, res_dir));
            }
        }
    }

    let cwd = std::env::current_dir().ok()?;
    let candidates = [
        cwd.join("pi-gpui"),
        cwd.clone(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    ];

    for dir in candidates {
        let script = dir.join("sidecar").join("api-sidecar.mjs");
        let mjs = dir.join("dist-sidecar").join("server.mjs");
        let cjs = dir.join("dist-sidecar").join("server.cjs");
        let bundle = if mjs.exists() { mjs } else { cjs };
        if script.exists() && bundle.exists() {
            let repo_root = dir.parent().map(Path::to_path_buf).unwrap_or(cwd.clone());
            return Some((script, bundle, repo_root));
        }
    }
    None
}
