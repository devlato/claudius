//! Updates: a newer release found on GitHub, downloaded in the background, checked against the release's SHA256SUMS,
//! and installed the way this claudius was installed (the Mac app, an AppImage, the install script's folder, the
//! Windows installer) when the person says so. Packages and builds update their own way, which is said instead.
//! The studio checks once a day while automatic updates are on (`claudius update --auto off`, the studio's ? menu,
//! `CLAUDIUS_UPDATES=off`); `claudius update` checks now. docs/src/guide/updates.md describes it.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const REPO: &str = "https://github.com/devlato/claudius";

/// how this claudius was installed, which decides how it updates
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
    /// the Mac app, at this path
    MacApp(PathBuf),
    /// an AppImage, at this path
    AppImage(PathBuf),
    /// the programs the install script put in this folder, with the web studio in ../share/claudius/web
    Folder(PathBuf),
    /// the folder of the Windows installer (and of install.ps1), which the installer updates
    Windows(PathBuf),
    /// installed some other way, which updates it: how
    Elsewhere(String),
}

/// what the updater is doing, or found
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    /// this is the latest release
    Current,
    Downloading {
        version: String,
        progress: f32,
    },
    /// downloaded and checked, ready to install
    Ready {
        version: String,
        file: PathBuf,
    },
    /// a newer release, which this install gets another way: how
    Elsewhere {
        version: String,
        how: String,
    },
    Failed(String),
}

static STATUS: Mutex<Status> = Mutex::new(Status::Idle);

pub fn status() -> Status {
    STATUS.lock().map(|s| s.clone()).unwrap_or(Status::Idle)
}

fn set(s: Status) {
    if let Ok(mut g) = STATUS.lock() {
        *g = s;
    }
}

/// the version this claudius is, or `CLAUDIUS_UPDATE_SINCE` to try the updater against the latest release
fn current() -> String {
    std::env::var("CLAUDIUS_UPDATE_SINCE")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into())
}

/// the version of a release from the address GitHub sends for the latest one (`.../releases/tag/v0.3.0`)
fn version_from(location: &str) -> Option<String> {
    let v = location.rsplit('/').next()?.strip_prefix('v')?;
    let parts: Vec<&str> = v.split('.').collect();
    (parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())))
        .then(|| v.to_string())
}

/// is `latest` (a release, x.y.z) newer than `current` (which may be a pre-release of it)?
fn newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Option<([u64; 3], bool)> {
        let (core, pre) = v.split_once('-').map_or((v, false), |(c, _)| (c, true));
        let mut n = core.split('.').map(|p| p.parse::<u64>().ok());
        Some(([n.next()??, n.next()??, n.next()??], pre))
    };
    match (parse(current), parse(latest)) {
        (Some((c, pre)), Some((l, _))) => l > c || (l == c && pre),
        _ => false,
    }
}

/// the target triple of this build, as the release names its files
fn target() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        os => format!("{arch}-{os}"),
    }
}

/// how a claudius at `exe` (canonical) was installed; `appimage` is `$APPIMAGE`
fn install_of(exe: &Path, appimage: Option<&Path>, home: &Path) -> Install {
    if let Some(a) = appimage {
        return Install::AppImage(a.to_path_buf());
    }
    let text = exe.to_string_lossy().replace('\\', "/");
    if let Some(app) = exe.ancestors().find(|a| a.extension().is_some_and(|x| x == "app")) {
        if crate::connect::temporary(exe).is_some() {
            return Install::Elsewhere(
                "move Claudius into Applications and open it from there, and it updates itself".into(),
            );
        }
        return Install::MacApp(app.to_path_buf());
    }
    if text.contains("/Cellar/") || text.starts_with("/opt/homebrew/") || text.starts_with("/home/linuxbrew/") {
        return Install::Elsewhere("Homebrew updates it: brew upgrade claudius".into());
    }
    if exe.starts_with(home.join(".cargo")) {
        return Install::Elsewhere("cargo installed it: install it again from a newer checkout".into());
    }
    if text.contains("/target/debug/") || text.contains("/target/release/") {
        return Install::Elsewhere("this is a build from source: pull, and build it again".into());
    }
    if text.starts_with("/usr/") {
        return Install::Elsewhere(
            "its package updates it: sudo apt upgrade claudius, or sudo dnf upgrade claudius".into(),
        );
    }
    let Some(bin) = exe.parent() else { return Install::Elsewhere(downloads()) };
    if text.to_ascii_lowercase().contains("/programs/claudius/bin/") {
        return Install::Windows(bin.to_path_buf());
    }
    if bin.join("../share/claudius/web/index.html").exists() {
        return Install::Folder(bin.to_path_buf());
    }
    Install::Elsewhere(downloads())
}

fn downloads() -> String {
    format!("download it from {REPO}/releases")
}

pub fn install() -> Result<Install> {
    let exe = std::env::current_exe().context("cannot find this program")?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let home = dirs::home_dir().unwrap_or_default();
    Ok(install_of(&exe, std::env::var_os("APPIMAGE").map(PathBuf::from).as_deref(), &home))
}

/// the release's files for `install`, the one wanted first
fn assets(install: &Install, version: &str) -> Vec<String> {
    let t = target();
    match install {
        Install::MacApp(_) => {
            vec![format!("claudius-{version}-{t}.dmg"), format!("claudius-{version}-universal-apple-darwin.dmg")]
        }
        Install::AppImage(_) => vec![format!("claudius-{version}-{t}.AppImage")],
        Install::Folder(_) => {
            let headless = if cfg!(feature = "studio") { "" } else { "-headless" };
            vec![format!("claudius-{version}-{t}{headless}.tar.gz")]
        }
        Install::Windows(_) => vec![format!("claudius-{version}-{t}.msi")],
        Install::Elsewhere(_) => Vec::new(),
    }
}

/// the SHA-256 that a SHA256SUMS file gives for `name`
fn sum_of(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (sum, file) = l.split_once(char::is_whitespace)?;
        (file.trim_start_matches([' ', '*']) == name).then(|| sum.to_ascii_lowercase())
    })
}

fn agent(timeout: Duration, redirects: u32) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .max_redirects(redirects)
        .http_status_as_error(redirects > 0)
        .user_agent(format!("claudius/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// the latest release's version, from where GitHub's `releases/latest` points (no API, so no rate limit)
fn latest() -> Result<String> {
    let resp = agent(Duration::from_secs(15), 0).get(&format!("{REPO}/releases/latest")).call()?;
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .context("GitHub did not say which release is the latest")?;
    version_from(location).with_context(|| format!("cannot read a version in {location}"))
}

/// downloads `url` into `dest`, and its SHA-256
fn fetch(url: &str, dest: &Path, progress: &dyn Fn(f32)) -> Result<String> {
    let resp = agent(Duration::from_secs(900), 10).get(url).call().with_context(|| format!("cannot download {url}"))?;
    let total: u64 = resp.headers().get("content-length").and_then(|v| v.to_str().ok()?.parse().ok()).unwrap_or(0);
    let part = dest.with_extension("part");
    let mut out = std::fs::File::create(&part).with_context(|| format!("cannot write {}", part.display()))?;
    let mut reader = resp.into_body().into_reader();
    let (mut hasher, mut done, mut buf) = (Sha256::new(), 0u64, vec![0u8; 1 << 16]);
    loop {
        let n = reader.read(&mut buf).with_context(|| format!("the download of {url} stopped"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if total > 0 {
            progress(done as f32 / total as f32);
        }
    }
    out.flush()?;
    std::fs::rename(&part, dest)?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let (mut hasher, mut buf) = (Sha256::new(), vec![0u8; 1 << 16]);
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// looks for a newer release and, when this install can take it, downloads and checks it
pub fn check() -> Result<Status> {
    let latest = latest()?;
    if !newer(&current(), &latest) {
        return Ok(Status::Current);
    }
    let install = install()?;
    if let Install::Elsewhere(how) = install {
        return Ok(Status::Elsewhere { version: latest, how });
    }
    let base = format!("{REPO}/releases/download/v{latest}");
    let sums =
        agent(Duration::from_secs(30), 10).get(&format!("{base}/SHA256SUMS")).call()?.body_mut().read_to_string()?;
    let Some((asset, sum)) = assets(&install, &latest).into_iter().find_map(|a| sum_of(&sums, &a).map(|s| (a, s)))
    else {
        return Ok(Status::Elsewhere { version: latest, how: downloads() });
    };
    let cache = claudius::config::cache_dir("update");
    // the downloads of other versions go
    for old in std::fs::read_dir(&cache).into_iter().flatten().flatten() {
        if old.file_name() != latest.as_str() {
            let _ = std::fs::remove_dir_all(old.path());
        }
    }
    let dir = cache.join(&latest);
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(&asset);
    if !(file.is_file() && sha256_file(&file)? == sum) {
        let progress = |p: f32| set(Status::Downloading { version: latest.clone(), progress: p });
        progress(0.0);
        if fetch(&format!("{base}/{asset}"), &file, &progress)? != sum {
            let _ = std::fs::remove_file(&file);
            bail!("the download of {asset} does not match the release's SHA256SUMS, so it was thrown away");
        }
    }
    Ok(Status::Ready { version: latest, file })
}

fn settings_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("claudius").join("updates.json"))
}

fn settings() -> Map<String, Value> {
    settings_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn save(m: &Map<String, Value>) -> Result<()> {
    let path = settings_file().context("no configuration folder to keep the setting in")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(m)? + "\n")
        .with_context(|| format!("cannot write {}", path.display()))
}

/// what in the environment keeps automatic updates off
pub fn vetoed() -> Option<&'static str> {
    let var = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_ascii_lowercase());
    if var("CLAUDIUS_UPDATES").is_some_and(|v| matches!(v.as_str(), "0" | "off" | "false" | "no")) {
        Some("CLAUDIUS_UPDATES is off")
    } else if var("CI").is_some_and(|v| !matches!(v.as_str(), "" | "0" | "false")) {
        Some("it runs in CI")
    } else {
        None
    }
}

/// automatic updates: on unless turned off
pub fn automatic() -> bool {
    vetoed().is_none() && settings().get("automatic").and_then(Value::as_bool) != Some(false)
}

pub fn set_automatic(on: bool) -> Result<()> {
    let mut m = settings();
    m.insert("automatic".into(), Value::Bool(on));
    save(&m)
}

fn today() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400)
}

fn run(manual: bool) {
    set(Status::Checking);
    let found = check().unwrap_or_else(|e| Status::Failed(format!("{e:#}")));
    if !manual {
        let mut m = settings();
        m.insert("checked".into(), Value::from(today()));
        let _ = save(&m);
    }
    set(found);
}

/// in the background, for the studio: once a day, while automatic updates are on
pub fn start() {
    if !automatic() || settings().get("checked").and_then(Value::as_u64) == Some(today()) {
        return;
    }
    std::thread::spawn(|| run(false));
}

/// in the background, now (the studio's "Check for updates")
pub fn check_now() {
    if !matches!(status(), Status::Checking | Status::Downloading { .. }) {
        std::thread::spawn(|| run(true));
    }
}

/// a shell that waits for this process to end, removes `old`, and then runs `then` (when it is not empty)
#[cfg(unix)]
fn after_exit(old: Option<&Path>, then: &[OsString]) -> Result<()> {
    if old.is_none() && then.is_empty() {
        return Ok(());
    }
    const SCRIPT: &str = r#"pid=$1; old=$2; shift 2
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
if [ -n "$old" ]; then rm -rf "$old"; fi
if [ $# -gt 0 ]; then exec "$@"; fi"#;
    let mut args: Vec<OsString> = vec!["-c".into(), SCRIPT.into(), "sh".into(), std::process::id().to_string().into()];
    args.push(old.map(|p| p.as_os_str().to_owned()).unwrap_or_default());
    args.extend(then.iter().cloned());
    std::process::Command::new("/bin/sh")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("cannot start the step that finishes the update")?;
    Ok(())
}

/// installs the downloaded update. `reopen` is the studio's: the new version opens once this process ends, which the
/// caller then makes happen. On the command line the update is in place when this returns.
pub fn apply(reopen: bool) -> Result<String> {
    let Status::Ready { version, file } = status() else { bail!("no update has been downloaded") };
    match install()? {
        Install::MacApp(app) => swap_app(&file, &app, reopen)?,
        Install::AppImage(path) => swap_appimage(&file, &path, reopen)?,
        Install::Folder(bin) => swap_folder(&file, &bin, reopen)?,
        Install::Windows(bin) => run_installer(&file, &bin, reopen)?,
        Install::Elsewhere(how) => bail!(how),
    }
    set(Status::Idle);
    Ok(version)
}

/// the disk image's app into the place of `app`, the old one removed once this process ends
#[cfg(unix)]
fn swap_app(dmg: &Path, app: &Path, reopen: bool) -> Result<()> {
    let parent = app.parent().context("the app has no folder")?;
    let mount = dmg.with_extension("mount");
    let _ = std::fs::remove_dir_all(&mount);
    std::fs::create_dir_all(&mount)?;
    let attached = std::process::Command::new("hdiutil")
        .args(["attach", "-quiet", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mount)
        .arg(dmg)
        .status()?;
    if !attached.success() {
        bail!("cannot open {}", dmg.display());
    }
    let copied = (|| -> Result<PathBuf> {
        let found = std::fs::read_dir(&mount)?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "app"))
            .context("the disk image has no app")?;
        let new = parent.join(".Claudius.app.new");
        let _ = std::fs::remove_dir_all(&new);
        let ok = std::process::Command::new("ditto").arg(&found).arg(&new).status()?;
        if !ok.success() {
            bail!("cannot copy the new app into {} (is it a folder you can write in?)", parent.display());
        }
        Ok(parent.join(found.file_name().context("no name")?))
    })();
    let _ = std::process::Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount).status();
    let _ = std::fs::remove_dir(&mount);
    let target = copied?;
    let old = parent.join(".Claudius.app.old");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(app, &old).with_context(|| format!("cannot move {} aside", app.display()))?;
    if let Err(e) = std::fs::rename(parent.join(".Claudius.app.new"), &target) {
        let _ = std::fs::rename(&old, app);
        return Err(e).context("cannot put the new app in place, so the old one is back");
    }
    let then: Vec<OsString> = if reopen { vec!["/usr/bin/open".into(), target.into_os_string()] } else { Vec::new() };
    after_exit(Some(&old), &then)
}

#[cfg(not(unix))]
fn swap_app(_: &Path, _: &Path, _: bool) -> Result<()> {
    bail!("the Mac app updates only on a Mac")
}

/// the downloaded AppImage in the place of the running one
#[cfg(unix)]
fn swap_appimage(file: &Path, path: &Path, reopen: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let new = path.with_extension("AppImage.new");
    std::fs::copy(file, &new).with_context(|| format!("cannot write next to {}", path.display()))?;
    std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&new, path)?;
    let then: Vec<OsString> = if reopen { vec![path.as_os_str().to_owned()] } else { Vec::new() };
    after_exit(None, &then)
}

#[cfg(not(unix))]
fn swap_appimage(_: &Path, _: &Path, _: bool) -> Result<()> {
    bail!("AppImages update only on Linux")
}

/// the archive's programs and web studio in the place of the install script's
#[cfg(unix)]
fn swap_folder(archive: &Path, bin: &Path, reopen: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let unpacked = archive.with_extension("unpacked");
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked)?;
    let ok = std::process::Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(&unpacked).status()?;
    if !ok.success() {
        bail!("cannot unpack {}", archive.display());
    }
    let root = std::fs::read_dir(&unpacked)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .context("the archive is empty")?;
    for file in std::fs::read_dir(root.join("bin"))?.flatten() {
        let name = file.file_name();
        // a new file moved into place, so that the running program keeps its old one
        let new = bin.join(format!(".{}.new", name.to_string_lossy()));
        std::fs::copy(file.path(), &new).with_context(|| format!("cannot write in {}", bin.display()))?;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
        std::fs::rename(&new, bin.join(&name))?;
    }
    let web = bin.join("../share/claudius/web");
    let new_web = bin.join("../share/claudius/.web.new");
    let _ = std::fs::remove_dir_all(&new_web);
    let ok = std::process::Command::new("cp").arg("-R").arg(root.join("share/claudius/web")).arg(&new_web).status()?;
    if ok.success() {
        let _ = std::fs::remove_dir_all(&web);
        std::fs::rename(&new_web, &web)?;
    }
    let _ = std::fs::remove_dir_all(&unpacked);
    let then: Vec<OsString> =
        if reopen { vec![bin.join("claudius").into_os_string(), "studio".into()] } else { Vec::new() };
    after_exit(None, &then)
}

#[cfg(not(unix))]
fn swap_folder(_: &Path, _: &Path, _: bool) -> Result<()> {
    bail!("the install script's folder updates only on macOS and Linux")
}

/// the downloaded installer, run once this process ends (it updates the programs this one is), and the studio again
#[cfg(windows)]
fn run_installer(msi: &Path, bin: &Path, reopen: bool) -> Result<()> {
    let quote = |p: &Path| p.display().to_string().replace('\'', "''");
    let mut script = format!(
        "Wait-Process -Id {} -ErrorAction SilentlyContinue; Start-Process msiexec.exe -Wait -ArgumentList '/i', '\"{}\"', '/passive', '/norestart'",
        std::process::id(),
        quote(msi)
    );
    if reopen {
        script.push_str(&format!("; Start-Process '{}'", quote(&bin.join("claudius-studio.exe"))));
    }
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .spawn()
        .context("cannot start the installer")?;
    Ok(())
}

#[cfg(not(windows))]
fn run_installer(_: &Path, _: &Path, _: bool) -> Result<()> {
    bail!("the Windows installer runs only on Windows")
}

/// `claudius update`
pub fn command(check_only: bool, yes: bool, auto: Option<bool>) -> Result<()> {
    if let Some(on) = auto {
        set_automatic(on)?;
        println!("automatic updates are {}", if on { "on: the studio checks once a day" } else { "off" });
        return Ok(());
    }
    eprintln!("claudius {}: looking for a newer release", current());
    if check_only {
        let latest = latest()?;
        if newer(&current(), &latest) {
            println!("claudius {latest} is out: `claudius update` installs it ({REPO}/releases/tag/v{latest})");
        } else {
            println!("claudius {} is the latest", current());
        }
        return Ok(());
    }
    set(Status::Checking);
    let found = check()?;
    set(found.clone());
    match found {
        Status::Current => println!("claudius {} is the latest", current()),
        Status::Elsewhere { version, how } => println!("claudius {version} is out: {how}"),
        Status::Ready { version, .. } => {
            if !yes && !crate::setup::confirm(&format!("Update to claudius {version}?"))? {
                return Ok(());
            }
            apply(false)?;
            println!("updated to claudius {version}; what it changed: {REPO}/releases/tag/v{version}");
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_release_comes_from_where_github_points() {
        assert_eq!(version_from("https://github.com/devlato/claudius/releases/tag/v0.3.0"), Some("0.3.0".into()));
        assert_eq!(version_from("https://github.com/devlato/claudius/releases/tag/0.3.0"), None);
        assert_eq!(version_from("https://github.com/devlato/claudius/releases/tag/v0.3"), None);
        assert_eq!(version_from("https://github.com/devlato/claudius/releases/tag/v0.3.0;rm"), None);
    }

    #[test]
    fn only_a_newer_release_is_an_update() {
        assert!(newer("0.2.0", "0.3.0") && newer("0.2.9", "0.10.0") && newer("1.0.0-rc.1", "1.0.0"));
        assert!(!newer("0.3.0", "0.3.0") && !newer("0.4.0", "0.3.0") && !newer("garbage", "0.3.0"));
    }

    #[test]
    fn each_install_updates_its_own_way() {
        let home = Path::new("/Users/me");
        let app = Path::new("/Applications/Claudius.app/Contents/MacOS/Claudius");
        assert_eq!(install_of(app, None, home), Install::MacApp("/Applications/Claudius.app".into()));
        let image = Path::new("/home/me/Apps/claudius.AppImage");
        assert_eq!(
            install_of(Path::new("/tmp/.mount_x/usr/bin/claudius"), Some(image), home),
            Install::AppImage(image.into())
        );
        let elsewhere = |exe: &str| matches!(install_of(Path::new(exe), None, home), Install::Elsewhere(_));
        assert!(elsewhere("/opt/homebrew/Cellar/claudius/0.2.0/bin/claudius"));
        assert!(elsewhere("/Users/me/.cargo/bin/claudius"));
        assert!(elsewhere("/Users/me/hobby/claudius/target/release/claudius"));
        assert!(elsewhere("/usr/bin/claudius"));
        assert!(elsewhere("/Volumes/Claudius 0.3.0/Claudius.app/Contents/MacOS/Claudius"));
        let windows = Path::new(r"C:\Users\me\AppData\Local\Programs\claudius\bin\claudius.exe");
        assert!(matches!(install_of(windows, None, home), Install::Windows(_)));
    }

    #[test]
    fn the_release_file_is_the_one_for_this_install() {
        let t = target();
        assert_eq!(assets(&Install::MacApp("/A".into()), "0.3.0")[0], format!("claudius-0.3.0-{t}.dmg"));
        assert_eq!(assets(&Install::AppImage("/a".into()), "0.3.0"), [format!("claudius-0.3.0-{t}.AppImage")]);
        assert!(assets(&Install::Elsewhere("x".into()), "0.3.0").is_empty());
        let sums = "abc  claudius-0.3.0-x.tar.gz\nDEF *claudius-0.3.0-x.dmg\n";
        assert_eq!(sum_of(sums, "claudius-0.3.0-x.dmg"), Some("def".into()));
        assert_eq!(sum_of(sums, "claudius-0.3.0-x.msi"), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_install_script_s_folder_takes_a_new_archive() {
        let dir = std::env::temp_dir().join(format!("claudius-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src/claudius-9.9.9-x");
        std::fs::create_dir_all(src.join("bin")).unwrap();
        std::fs::create_dir_all(src.join("share/claudius/web")).unwrap();
        std::fs::write(src.join("bin/claudius"), "new").unwrap();
        std::fs::write(src.join("share/claudius/web/index.html"), "new web").unwrap();
        let archive = dir.join("a.tar.gz");
        assert!(
            std::process::Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(dir.join("src"))
                .arg("claudius-9.9.9-x")
                .status()
                .unwrap()
                .success()
        );
        let bin = dir.join("prefix/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(dir.join("prefix/share/claudius/web")).unwrap();
        std::fs::write(bin.join("claudius"), "old").unwrap();
        std::fs::write(dir.join("prefix/share/claudius/web/index.html"), "old web").unwrap();
        swap_folder(&archive, &bin, false).unwrap();
        assert_eq!(std::fs::read_to_string(bin.join("claudius")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(dir.join("prefix/share/claudius/web/index.html")).unwrap(), "new web");
        std::fs::remove_dir_all(&dir).ok();
    }
}
