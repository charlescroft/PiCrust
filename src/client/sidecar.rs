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

        let active_port = find_available_port(port);
        let node_bin = resolve_node_binary();
        let mut cmd = Command::new(node_bin);
        cmd.arg(&sidecar_script)
            .current_dir(&repo_root)
            .env("PI_SIDECAR_HOST", "127.0.0.1")
            .env("PI_SIDECAR_PORT", active_port.to_string())
            .env("PI_DESKTOP_API_TOKEN", &token)
            .env("PI_DESKTOP_INSTANCE_ID", &instance_id)
            .env("PI_PARENT_PID", std::process::id().to_string())
            .env("PI_SIDECAR_BUNDLE", &bundle_path)
            .env("PI_WEB_IDLE_TIMEOUT_MS", "0")
            .env("NO_PROXY", "localhost,127.0.0.1,::1")
            .env("no_proxy", "localhost,127.0.0.1,::1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.stderr(Stdio::null());
        }
        #[cfg(not(target_os = "windows"))]
        {
            cmd.stderr(Stdio::null());
        }

        match cmd.spawn() {
            Ok(child) => Some(Self {
                child: Some(child),
                port: active_port,
                token,
                instance_id,
            }),
            Err(_) => None,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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

fn find_available_port(preferred: u16) -> u16 {
    if std::net::TcpListener::bind(("127.0.0.1", preferred)).is_ok() {
        return preferred;
    }
    for offset in 1..=30 {
        if std::net::TcpListener::bind(("127.0.0.1", preferred + offset)).is_ok() {
            return preferred + offset;
        }
    }
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .map(|addr| addr.port())
        .unwrap_or(preferred)
}

fn resolve_node_binary() -> PathBuf {
    let bin_name = if cfg!(windows) { "node.exe" } else { "node" };

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(bin_name);
            if p.is_file() {
                return p;
            }
        }
    }

    let home = crate::client::native_index::user_home();
    let mut candidates = vec![
        home.join(".volta").join("bin").join(bin_name),
        home.join(".local").join("share").join("fnm").join("aliases").join("default").join("bin").join(bin_name),
    ];

    #[cfg(target_os = "windows")]
    {
        if let Ok(pf) = std::env::var("ProgramFiles") {
            candidates.push(PathBuf::from(pf).join("nodejs").join("node.exe"));
        }
        if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(pf86).join("nodejs").join("node.exe"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(&local).join("Programs").join("nodejs").join("node.exe"));
            candidates.push(PathBuf::from(&local).join("fnm_multishells").join("node.exe"));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            candidates.push(PathBuf::from(appdata).join("nvm").join("node.exe"));
        }
        candidates.push(PathBuf::from(r"C:\Program Files\nodejs\node.exe"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        candidates.extend([
            PathBuf::from("/opt/homebrew/bin/node"),
            PathBuf::from("/usr/local/bin/node"),
            PathBuf::from("/usr/bin/node"),
            PathBuf::from(
                "/Applications/Pi Agent.app/Contents/Resources/resources/Pi Agent Server.app/Contents/MacOS/node",
            ),
        ]);
        // Check ~/.nvm/versions/node/*/bin/node
        let nvm_dir = home.join(".nvm").join("versions").join("node");
        if let Ok(entries) = std::fs::read_dir(nvm_dir) {
            let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            versions.sort();
            if let Some(latest) = versions.last() {
                candidates.insert(0, latest.join("bin").join("node"));
            }
        }
    }

    for p in candidates {
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from(bin_name)
}

fn check_sidecar_dir(res_dir: &Path) -> Option<(PathBuf, PathBuf, PathBuf)> {
    let script = res_dir.join("api-sidecar.mjs");
    let mjs = res_dir.join("server.mjs");
    let cjs = res_dir.join("server.cjs");
    let bundle = if mjs.exists() { mjs } else { cjs };
    if script.exists() && bundle.exists() {
        return Some((script, bundle, res_dir.to_path_buf()));
    }
    None
}

fn locate_sidecar() -> Option<(PathBuf, PathBuf, PathBuf)> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // 1. Windows / portable layout: <exe_dir>/resources/sidecar or <exe_dir>/sidecar
            for sub in [
                exe_dir.join("resources").join("sidecar"),
                exe_dir.join("Resources").join("sidecar"),
                exe_dir.join("sidecar"),
            ] {
                if let Some(found) = check_sidecar_dir(&sub) {
                    return Some(found);
                }
            }
            // 2. macOS .app (<exe_dir>/../Resources/sidecar) & Linux AppImage (<exe_dir>/../resources/sidecar)
            if let Some(parent) = exe_dir.parent() {
                for sub in [
                    parent.join("Resources").join("sidecar"),
                    parent.join("resources").join("sidecar"),
                ] {
                    if let Some(found) = check_sidecar_dir(&sub) {
                        return Some(found);
                    }
                }
            }
        }
    }

    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.clone());
        roots.push(cwd.join("pi-gpui"));
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    for dir in roots {
        let script = dir.join("sidecar").join("api-sidecar.mjs");
        let mjs = dir.join("dist-sidecar").join("server.mjs");
        let cjs = dir.join("dist-sidecar").join("server.cjs");
        let bundle = if mjs.exists() { mjs } else { cjs };
        if script.exists() && bundle.exists() {
            let repo_root = if dir.join("node_modules").exists() || dir.join("Cargo.toml").exists()
            {
                dir.clone()
            } else {
                dir.parent().map(Path::to_path_buf).unwrap_or(dir.clone())
            };
            return Some((script, bundle, repo_root));
        }
    }
    None
}
