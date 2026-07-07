use anyhow::{Result, anyhow};
use dw_config::{WorkflowConfig, load_user_settings, load_workflow_config, resolve_root};
use dw_core::{
    ByteCount, ExecutablePath, GitCommitSha, RuntimeIdentifier, SemanticVersion, Sha256Digest,
    UpgradeActionEvent, UpgradeAssetName, UpgradeFileName, UpgradeOwner, UpgradeReleaseTag,
    UpgradeRepositoryName,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const DEFAULT_OWNER: &str = "sachahjkl";
const DEFAULT_REPOSITORY: &str = "dw";
const DEFAULT_MANIFEST_ASSET: &str = "release.json";
const WINDOWS_PE_SIGNATURE: [u8; 2] = [0x4D, 0x5A];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateOptions {
    pub(crate) owner: UpgradeOwner,
    pub(crate) repository: UpgradeRepositoryName,
    pub(crate) include_prerelease: bool,
    pub(crate) asset_name: UpgradeAssetName,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    #[serde(rename = "tag_name")]
    tag_name: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    #[serde(rename = "browser_download_url")]
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseManifest {
    version: String,
    commit: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    rid: String,
    #[serde(rename = "fileName")]
    file_name: String,
    sha256: String,
    #[serde(default)]
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpgradeReport {
    Check(UpgradeCheckReport),
    Installed(UpgradeInstallReport),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeCheckReport {
    pub release_tag: UpgradeReleaseTag,
    pub version: SemanticVersion,
    pub commit: GitCommitSha,
    pub assets: Vec<UpgradeAssetSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeAssetSummary {
    pub rid: RuntimeIdentifier,
    pub file_name: UpgradeFileName,
    pub sha256: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeInstallReport {
    pub version: SemanticVersion,
    pub commit: GitCommitSha,
    pub executable_path: ExecutablePath,
    pub deferred_windows_replacement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementReport {
    executable_path: PathBuf,
    deferred_windows_replacement: bool,
}

pub struct UpgradeActionRun {
    pub events: mpsc::Receiver<UpgradeActionEvent>,
    pub result: JoinHandle<Result<UpgradeReport>>,
}

pub fn spawn_upgrade(check: bool, rid: Option<RuntimeIdentifier>) -> UpgradeActionRun {
    let (sender, receiver) = mpsc::channel(32);
    let result = tokio::spawn(async move { handle_upgrade_with_events(check, rid, sender).await });
    UpgradeActionRun {
        events: receiver,
        result,
    }
}

pub async fn handle_upgrade(check: bool, rid: Option<RuntimeIdentifier>) -> Result<UpgradeReport> {
    let (sender, _receiver) = mpsc::channel(1);
    handle_upgrade_with_events(check, rid, sender).await
}

async fn handle_upgrade_with_events(
    check: bool,
    rid: Option<RuntimeIdentifier>,
    events: mpsc::Sender<UpgradeActionEvent>,
) -> Result<UpgradeReport> {
    emit_upgrade_event(&events, UpgradeActionEvent::CheckingHost).await;
    ensure_supported_host(std::env::current_exe().ok().as_deref())?;
    emit_upgrade_event(&events, UpgradeActionEvent::ResolvingConfig).await;
    let root = resolve_root(load_user_settings().root.as_deref());
    let workflow = load_workflow_config(&root);
    let options = resolve_updates(&workflow)?;
    emit_upgrade_event(
        &events,
        UpgradeActionEvent::FetchingRelease {
            owner: options.owner.clone(),
            repository: options.repository.clone(),
        },
    )
    .await;
    let client = reqwest::Client::builder().user_agent("dw/1.0").build()?;
    let release = get_latest_release(&client, &options).await?;
    emit_upgrade_event(
        &events,
        UpgradeActionEvent::FetchingManifest {
            asset_name: options.asset_name.clone(),
        },
    )
    .await;
    let manifest = download_manifest(&client, &release, &options.asset_name).await?;

    if check {
        emit_upgrade_event(
            &events,
            UpgradeActionEvent::Completed {
                version: SemanticVersion::from(manifest.version.clone()),
            },
        )
        .await;
        return Ok(UpgradeReport::Check(upgrade_check_report(
            &release, &manifest,
        )));
    }

    let rid = rid.unwrap_or_else(|| RuntimeIdentifier::from(default_rid()));
    run_upgrade(&client, &manifest, &rid, &events).await
}

pub(crate) fn resolve_updates(workflow: &WorkflowConfig) -> Result<UpdateOptions> {
    let value = workflow.updates.as_ref();
    let owner = string_property(value, "owner").unwrap_or_else(|| DEFAULT_OWNER.into());
    let repository =
        string_property(value, "repository").unwrap_or_else(|| DEFAULT_REPOSITORY.into());
    if owner.trim().is_empty() || repository.trim().is_empty() {
        return Err(anyhow!(
            "Missing updates.owner / updates.repository configuration in workflow.json."
        ));
    }
    Ok(UpdateOptions {
        owner: UpgradeOwner::from(owner),
        repository: UpgradeRepositoryName::from(repository),
        include_prerelease: value
            .and_then(|value| value.get("includePrerelease"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        asset_name: UpgradeAssetName::from(
            string_property(value, "assetName").unwrap_or_else(|| DEFAULT_MANIFEST_ASSET.into()),
        ),
    })
}

async fn get_latest_release(
    client: &reqwest::Client,
    options: &UpdateOptions,
) -> Result<GitHubRelease> {
    let url = if options.include_prerelease {
        format!(
            "https://api.github.com/repos/{}/{}/releases",
            options.owner, options.repository
        )
    } else {
        format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            options.owner, options.repository
        )
    };
    let response = client.get(url).send().await?;
    let status = response.status().as_u16();
    let body = response.text().await?;
    if !(200..300).contains(&status) {
        return Err(anyhow!("GitHub Releases HTTP {status}: {body}"));
    }
    if options.include_prerelease {
        let releases: Vec<GitHubRelease> = serde_json::from_str(&body)?;
        releases
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No GitHub release found."))
    } else {
        Ok(serde_json::from_str(&body)?)
    }
}

async fn download_manifest(
    client: &reqwest::Client,
    release: &GitHubRelease,
    asset_name: &UpgradeAssetName,
) -> Result<ReleaseManifest> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(asset_name.as_str()))
        .ok_or_else(|| anyhow!("Release asset not found: {asset_name}"))?;
    let response = client.get(&asset.browser_download_url).send().await?;
    let status = response.status().as_u16();
    let body = response.text().await?;
    if !(200..300).contains(&status) {
        return Err(anyhow!(
            "Could not download release.json HTTP {status}: {body}"
        ));
    }
    Ok(serde_json::from_str(&body)?)
}

async fn run_upgrade(
    client: &reqwest::Client,
    manifest: &ReleaseManifest,
    rid: &RuntimeIdentifier,
    events: &mpsc::Sender<UpgradeActionEvent>,
) -> Result<UpgradeReport> {
    emit_upgrade_event(
        events,
        UpgradeActionEvent::SelectingAsset { rid: rid.clone() },
    )
    .await;
    let asset = manifest
        .assets
        .iter()
        .find(|asset| asset.rid.eq_ignore_ascii_case(rid.as_str()))
        .ok_or_else(|| anyhow!("No asset for RID {rid}."))?;
    if asset.url.trim().is_empty() {
        return Err(anyhow!(
            "release.json must contain assets[].url to download an asset."
        ));
    }
    let executable_path = std::env::current_exe()?;
    emit_upgrade_event(
        events,
        UpgradeActionEvent::DownloadingAsset {
            file_name: UpgradeFileName::from(asset.file_name.clone()),
        },
    )
    .await;
    let temp_asset = download_asset(client, asset, events).await?;
    emit_upgrade_event(
        events,
        UpgradeActionEvent::VerifyingChecksum {
            file_name: UpgradeFileName::from(asset.file_name.clone()),
            expected_sha256: Sha256Digest::from(asset.sha256.clone()),
        },
    )
    .await;
    let hash_path = temp_asset.clone();
    let hash = tokio::task::spawn_blocking(move || file_sha256(&hash_path)).await??;
    if !hash.eq_ignore_ascii_case(&asset.sha256) {
        let _ = fs::remove_file(&temp_asset);
        return Err(anyhow!(
            "Invalid SHA256 for {}. Expected {}, got {}.",
            temp_asset.display(),
            asset.sha256,
            hash
        ));
    }
    emit_upgrade_event(
        events,
        UpgradeActionEvent::PreparingExecutable {
            file_name: UpgradeFileName::from(asset.file_name.clone()),
            rid: rid.clone(),
        },
    )
    .await;
    let asset_file_name = asset.file_name.clone();
    let temp_asset_for_prepare = temp_asset.clone();
    let rid_for_prepare = rid.to_string();
    let replacement = tokio::task::spawn_blocking(move || {
        prepare_replacement_executable(&asset_file_name, &temp_asset_for_prepare, &rid_for_prepare)
    })
    .await??;
    emit_upgrade_event(
        events,
        UpgradeActionEvent::ReplacingExecutable {
            executable_path: ExecutablePath::from(executable_path.display().to_string()),
        },
    )
    .await;
    let replacement =
        tokio::task::spawn_blocking(move || replace_executable(&executable_path, &replacement))
            .await??;
    emit_upgrade_event(
        events,
        UpgradeActionEvent::Completed {
            version: SemanticVersion::from(manifest.version.clone()),
        },
    )
    .await;
    Ok(UpgradeReport::Installed(UpgradeInstallReport {
        version: SemanticVersion::from(manifest.version.clone()),
        commit: GitCommitSha::from(manifest.commit.clone()),
        executable_path: ExecutablePath::from(replacement.executable_path.display().to_string()),
        deferred_windows_replacement: replacement.deferred_windows_replacement,
    }))
}

async fn emit_upgrade_event(events: &mpsc::Sender<UpgradeActionEvent>, event: UpgradeActionEvent) {
    let _ = events.send(event).await;
}

fn upgrade_check_report(release: &GitHubRelease, manifest: &ReleaseManifest) -> UpgradeCheckReport {
    UpgradeCheckReport {
        release_tag: UpgradeReleaseTag::from(release.tag_name.clone()),
        version: SemanticVersion::from(manifest.version.clone()),
        commit: GitCommitSha::from(manifest.commit.clone()),
        assets: manifest
            .assets
            .iter()
            .map(|asset| UpgradeAssetSummary {
                rid: RuntimeIdentifier::from(asset.rid.clone()),
                file_name: UpgradeFileName::from(asset.file_name.clone()),
                sha256: Sha256Digest::from(asset.sha256.clone()),
            })
            .collect(),
    }
}

async fn download_asset(
    client: &reqwest::Client,
    asset: &ReleaseAsset,
    events: &mpsc::Sender<UpgradeActionEvent>,
) -> Result<PathBuf> {
    let mut response = client.get(&asset.url).send().await?;
    let status = response.status().as_u16();
    let total = response.content_length().map(ByteCount::from);
    if !(200..300).contains(&status) {
        return Err(anyhow!("Could not download upgrade HTTP {status}."));
    }
    let path = std::env::temp_dir().join(format!(
        "dw-upgrade-{}{}",
        std::process::id(),
        extension_suffix(&asset.file_name)
    ));
    let file_name = UpgradeFileName::from(asset.file_name.clone());
    let mut file = fs::File::create(&path)?;
    let mut received = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)?;
        received += chunk.len() as u64;
        emit_upgrade_event(
            events,
            UpgradeActionEvent::DownloadedAssetBytes {
                file_name: file_name.clone(),
                received: ByteCount::from(received),
                total,
            },
        )
        .await;
    }
    file.flush()?;
    Ok(path)
}

pub(crate) fn prepare_replacement_executable(
    asset_file_name: &str,
    asset_path: &Path,
    rid: &str,
) -> Result<PathBuf> {
    if asset_file_name.ends_with(".zip") {
        return extract_windows_executable(asset_path);
    }
    if asset_file_name.ends_with(".tar.gz") || asset_file_name.ends_with(".tgz") {
        return extract_unix_executable(asset_path, rid);
    }
    if asset_file_name.ends_with(".exe") {
        ensure_windows_executable(asset_path, asset_file_name)?;
    }
    Ok(asset_path.to_path_buf())
}

fn extract_windows_executable(archive_path: &Path) -> Result<PathBuf> {
    let destination =
        std::env::temp_dir().join(format!("dw-upgrade-{}.exe", unique_upgrade_suffix()));
    let result = (|| {
        let file = fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let Some(name) = Path::new(entry.name())
                .file_name()
                .and_then(|name| name.to_str())
            else {
                continue;
            };
            if !name.eq_ignore_ascii_case("dw.exe") {
                continue;
            }
            let mut output = fs::File::create(&destination)?;
            std::io::copy(&mut entry, &mut output)?;
            ensure_windows_executable(&destination, entry.name())?;
            return Ok(destination.clone());
        }
        Err(anyhow!("Invalid upgrade archive: dw.exe not found."))
    })();
    let _ = fs::remove_file(archive_path);
    if result.is_err() {
        let _ = fs::remove_file(&destination);
    }
    result
}

fn extract_unix_executable(archive_path: &Path, rid: &str) -> Result<PathBuf> {
    let destination = std::env::temp_dir().join(format!("dw-upgrade-{}", unique_upgrade_suffix()));
    let result = (|| {
        let file = fs::File::open(archive_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name != "dw" {
                continue;
            }
            if !entry.header().entry_type().is_file() {
                continue;
            }
            entry.unpack(&destination)?;
            ensure_unix_executable(&destination, "dw", rid)?;
            return Ok(destination.clone());
        }
        Err(anyhow!("Invalid upgrade archive: dw not found."))
    })();
    let _ = fs::remove_file(archive_path);
    if result.is_err() {
        let _ = fs::remove_file(&destination);
    }
    result
}

fn ensure_windows_executable(path: &Path, display_name: &str) -> Result<()> {
    let mut signature = [0_u8; 2];
    fs::File::open(path)?.read_exact(&mut signature)?;
    if signature != WINDOWS_PE_SIGNATURE {
        let _ = fs::remove_file(path);
        return Err(anyhow!(
            "Invalid upgrade asset: {display_name} is not a Windows executable."
        ));
    }
    Ok(())
}

fn ensure_unix_executable(path: &Path, display_name: &str, rid: &str) -> Result<()> {
    if rid.starts_with("win-") {
        return Err(anyhow!(
            "Invalid upgrade asset: {display_name} is not a Windows executable."
        ));
    }
    if !fs::metadata(path)?.is_file() {
        let _ = fs::remove_file(path);
        return Err(anyhow!(
            "Invalid upgrade asset: {display_name} is not a file."
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn replace_executable(executable_path: &Path, replacement: &Path) -> Result<ReplacementReport> {
    if cfg!(windows) {
        return replace_windows_executable(executable_path, replacement);
    }
    fs::copy(replacement, executable_path)?;
    let _ = fs::remove_file(replacement);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(executable_path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable_path, permissions)?;
    }
    Ok(ReplacementReport {
        executable_path: executable_path.to_path_buf(),
        deferred_windows_replacement: false,
    })
}

fn replace_windows_executable(
    executable_path: &Path,
    replacement: &Path,
) -> Result<ReplacementReport> {
    let script = std::env::temp_dir().join(format!("dw-upgrade-{}.cmd", unique_upgrade_suffix()));
    let backup = executable_path.with_extension(format!(
        "{}bak",
        executable_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!("{extension}."))
            .unwrap_or_default()
    ));
    let script_content = windows_replacement_script(
        &replacement.display().to_string(),
        &executable_path.display().to_string(),
        &backup.display().to_string(),
        std::process::id(),
    )
    .replace('\n', "\r\n");
    fs::write(&script, script_content)?;
    if !script.is_file() {
        return Err(anyhow!(
            "Windows replacement script not found after creation: {}",
            script.display()
        ));
    }
    let command = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    ProcessCommand::new(&command)
        .arg("/d")
        .arg("/c")
        .arg(windows_replacement_command(&script))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            anyhow!(
                "Could not launch Windows replacement script {} via {}: {error}",
                script.display(),
                PathBuf::from(&command).display()
            )
        })?;
    Ok(ReplacementReport {
        executable_path: executable_path.to_path_buf(),
        deferred_windows_replacement: true,
    })
}

pub(crate) fn windows_replacement_script(
    replacement: &str,
    executable_path: &str,
    backup: &str,
    pid: u32,
) -> String {
    format!(
        r#"@echo off
setlocal EnableExtensions EnableDelayedExpansion
set "NEW={replacement}"
set "TARGET={executable_path}"
set "BACKUP={backup}"
set "PID={pid}"
set "ATTEMPTS=60"

:wait
tasklist /FI "PID eq %PID%" 2>nul | find "%PID%" >nul
if not errorlevel 1 (
  timeout /t 1 /nobreak >nul
  goto wait
)

if not exist "%NEW%" exit /b 1

:replace
if exist "%BACKUP%" del /f /q "%BACKUP%" >nul 2>nul
if exist "%TARGET%" (
  move /Y "%TARGET%" "%BACKUP%" >nul 2>nul
  if errorlevel 1 goto retry
)
copy /Y "%NEW%" "%TARGET%" >nul 2>nul
if errorlevel 1 (
  if exist "%BACKUP%" move /Y "%BACKUP%" "%TARGET%" >nul 2>nul
  goto retry
)
if not exist "%TARGET%" (
  if exist "%BACKUP%" move /Y "%BACKUP%" "%TARGET%" >nul 2>nul
  goto retry
)
del /f /q "%NEW%" >nul 2>nul
del /f /q "%BACKUP%" >nul 2>nul
del /f /q "%~f0" >nul 2>nul
exit /b 0

:retry
set /a ATTEMPTS-=1
if !ATTEMPTS! GTR 0 (
  timeout /t 1 /nobreak >nul
  goto replace
)
exit /b 1
"#
    )
}

pub(crate) fn windows_replacement_command(script: &Path) -> OsString {
    script
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| script.as_os_str().to_os_string())
}

fn unique_upgrade_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

pub(crate) fn ensure_supported_host(executable_path: Option<&Path>) -> Result<()> {
    if executable_path
        .and_then(Path::to_str)
        .is_some_and(|path| path.contains("/nix/store/"))
    {
        return Err(anyhow!(
            "Auto-update is unavailable for Nix installations. Use an explicit Nix refresh or Nix profile update."
        ));
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn string_property(value: Option<&serde_json::Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn default_rid() -> String {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "win-x64",
        ("linux", "x86_64") => "linux-x64",
        ("macos", "aarch64") => "osx-arm64",
        ("macos", "x86_64") => "osx-x64",
        _ => std::env::consts::ARCH,
    }
    .into()
}

fn extension_suffix(file_name: &str) -> String {
    if file_name.ends_with(".tar.gz") {
        ".tar.gz".into()
    } else {
        Path::new(file_name)
            .extension()
            .and_then(|value| value.to_str())
            .map(|extension| format!(".{extension}"))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_updates_falls_back_to_defaults() {
        let options = resolve_updates(&WorkflowConfig::default()).expect("updates");

        assert_eq!(options.owner, UpgradeOwner::from("sachahjkl"));
        assert_eq!(options.repository, UpgradeRepositoryName::from("dw"));
        assert!(!options.include_prerelease);
        assert_eq!(options.asset_name, UpgradeAssetName::from("release.json"));
    }

    #[test]
    fn resolve_updates_reads_workflow_override() {
        let workflow = WorkflowConfig {
            updates: Some(serde_json::json!({
                "owner": "owner",
                "repository": "repo",
                "includePrerelease": true,
                "assetName": "custom.json"
            })),
            ..WorkflowConfig::default()
        };

        let options = resolve_updates(&workflow).expect("updates");

        assert_eq!(options.owner, UpgradeOwner::from("owner"));
        assert_eq!(options.repository, UpgradeRepositoryName::from("repo"));
        assert!(options.include_prerelease);
        assert_eq!(options.asset_name, UpgradeAssetName::from("custom.json"));
    }

    #[test]
    fn ensure_supported_host_rejects_nix_store_path() {
        let error = ensure_supported_host(Some(Path::new("/nix/store/hash-dw/bin/dw")))
            .expect_err("nix path should fail");

        assert!(error.to_string().contains("Auto-update is unavailable"));
    }

    #[test]
    fn upgrade_check_report_preserves_manifest_summary_and_assets() {
        let release = GitHubRelease {
            tag_name: "v2026.07.03".into(),
            assets: Vec::new(),
        };
        let manifest = ReleaseManifest {
            version: "2026.07.03".into(),
            commit: "abcdef0".into(),
            assets: vec![ReleaseAsset {
                rid: "linux-x64".into(),
                file_name: "dw-linux-x64.tar.gz".into(),
                sha256: "hash".into(),
                url: "https://example.invalid/dw.tar.gz".into(),
            }],
        };

        let report = upgrade_check_report(&release, &manifest);

        assert_eq!(report.release_tag, UpgradeReleaseTag::from("v2026.07.03"));
        assert_eq!(report.version, SemanticVersion::from("2026.07.03"));
        assert_eq!(report.commit, GitCommitSha::from("abcdef0"));
        assert_eq!(report.assets.len(), 1);
        assert_eq!(report.assets[0].rid, RuntimeIdentifier::from("linux-x64"));
        assert_eq!(
            report.assets[0].file_name,
            UpgradeFileName::from("dw-linux-x64.tar.gz")
        );
    }

    #[test]
    fn windows_replacement_script_waits_and_restores_backup_on_failure() {
        let script = windows_replacement_script("new.exe", "dw.exe", "dw.exe.bak", 1234);

        assert!(script.contains("tasklist /FI \"PID eq %PID%\""));
        assert!(script.contains("setlocal EnableExtensions EnableDelayedExpansion"));
        assert!(script.contains("set \"ATTEMPTS=60\""));
        assert!(script.contains("move /Y \"%TARGET%\" \"%BACKUP%\" >nul 2>nul"));
        assert!(script.contains("copy /Y \"%NEW%\" \"%TARGET%\" >nul 2>nul"));
        assert!(script.contains(":retry"));
        assert!(script.contains("if !ATTEMPTS! GTR 0"));
        assert!(script.contains("move /Y \"%BACKUP%\" \"%TARGET%\" >nul 2>nul"));
        assert!(!script.contains("move /Y \"new.exe\" \"dw.exe\""));
    }

    #[test]
    fn windows_replacement_command_uses_script_file_name() {
        let command = windows_replacement_command(Path::new("/tmp/dw-upgrade-123.cmd"));

        assert_eq!(command, OsString::from("dw-upgrade-123.cmd"));
    }

    #[test]
    fn prepare_replacement_extracts_dw_from_tar_gz() {
        let path = create_tar_gz(&[("dw", b"#!/bin/sh\necho dw\n".as_slice())]);

        let replacement = prepare_replacement_executable("dw-linux-x64.tar.gz", &path, "linux-x64")
            .expect("tar.gz should extract");

        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(&replacement).expect("replacement should exist"),
            "#!/bin/sh\necho dw\n"
        );
        let _ = fs::remove_file(replacement);
    }

    #[test]
    fn prepare_replacement_accepts_windows_executable() {
        let path = std::env::temp_dir().join(format!("dw-upgrade-test-{}.exe", std::process::id()));
        fs::write(&path, [0x4d, 0x5a, 0x01]).expect("asset should be written");

        let replacement =
            prepare_replacement_executable("dw.exe", &path, "win-x64").expect("exe should pass");

        assert_eq!(replacement, path);
        let _ = fs::remove_file(replacement);
    }

    #[test]
    fn prepare_replacement_extracts_dw_exe_from_zip() {
        let path = create_zip(&[("dw.exe", &[0x4d, 0x5a, 0x01, 0x02])]);

        let replacement = prepare_replacement_executable("dw-win-x64.zip", &path, "win-x64")
            .expect("zip should extract");

        assert!(!path.exists());
        assert_eq!(
            fs::read(&replacement).expect("replacement should exist"),
            vec![0x4d, 0x5a, 0x01, 0x02]
        );
        let _ = fs::remove_file(replacement);
    }

    #[test]
    fn prepare_replacement_rejects_zip_without_dw_exe() {
        let path = create_zip(&[("readme.txt", &[0x41])]);

        let error = prepare_replacement_executable("dw-win-x64.zip", &path, "win-x64")
            .expect_err("zip without dw.exe should fail");

        assert!(error.to_string().contains("dw.exe not found"));
        assert!(!path.exists());
    }

    #[test]
    fn prepare_replacement_rejects_zip_when_dw_exe_is_not_windows_executable() {
        let path = create_zip(&[("dw.exe", &[0x50, 0x4b, 0x03, 0x04])]);

        let error = prepare_replacement_executable("dw-win-x64.zip", &path, "win-x64")
            .expect_err("invalid dw.exe should fail");

        assert!(error.to_string().contains("is not a Windows executable"));
        assert!(!path.exists());
    }

    fn create_zip(entries: &[(&str, &[u8])]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("dw-upgrade-test-{}.zip", unique_upgrade_suffix()));
        let file = fs::File::create(&path).expect("zip file should be created");
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in entries {
            archive
                .start_file(name, options)
                .expect("zip entry should start");
            std::io::Write::write_all(&mut archive, content).expect("zip entry should be written");
        }
        archive.finish().expect("zip should finish");
        path
    }

    fn create_tar_gz(entries: &[(&str, &[u8])]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dw-upgrade-test-{}.tar.gz",
            unique_upgrade_suffix()
        ));
        let file = fs::File::create(&path).expect("tar.gz file should be created");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            archive
                .append_data(&mut header, *name, *content)
                .expect("tar entry should be written");
        }
        archive.finish().expect("tar should finish");
        path
    }
}
