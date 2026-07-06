use anyhow::{Result, anyhow};
use dw_config::{WorkflowConfig, load_user_settings, load_workflow_config, resolve_root};
use dw_core::{
    ExecutablePath, RuntimeIdentifier, SemanticVersion, Sha256Digest, UpgradeAssetName,
    UpgradeFileName, UpgradeOwner, UpgradeRepositoryName,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_OWNER: &str = "sachahjkl";
const DEFAULT_REPOSITORY: &str = "dw";
const DEFAULT_MANIFEST_ASSET: &str = "release.json";
const WINDOWS_PE_SIGNATURE: [u8; 2] = [0x4D, 0x5A];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateOptions {
    pub(crate) owner: String,
    pub(crate) repository: String,
    pub(crate) include_prerelease: bool,
    pub(crate) asset_name: String,
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
    pub release_tag: String,
    pub version: String,
    pub commit: String,
    pub assets: Vec<UpgradeAssetSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeAssetSummary {
    pub rid: String,
    pub file_name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeInstallReport {
    pub version: String,
    pub commit: String,
    pub executable_path: String,
    pub deferred_windows_replacement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementReport {
    executable_path: PathBuf,
    deferred_windows_replacement: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpgradeStep {
    CheckHost,
    ResolveConfig,
    FetchRelease,
    FetchManifest,
    SelectAsset,
    DownloadAsset,
    VerifyChecksum,
    PrepareExecutable,
    ReplaceExecutable,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpgradeEvent {
    CheckingHost,
    ResolvingConfig,
    FetchingRelease {
        owner: UpgradeOwner,
        repository: UpgradeRepositoryName,
    },
    FetchingManifest {
        asset_name: UpgradeAssetName,
    },
    ReleaseAvailable {
        version: SemanticVersion,
    },
    SelectingAsset {
        rid: RuntimeIdentifier,
    },
    DownloadingAsset {
        file_name: UpgradeFileName,
    },
    VerifyingChecksum {
        file_name: UpgradeFileName,
        expected_sha256: Sha256Digest,
    },
    PreparingExecutable {
        file_name: UpgradeFileName,
        rid: RuntimeIdentifier,
    },
    ReplacingExecutable {
        executable_path: ExecutablePath,
    },
    Installed {
        version: SemanticVersion,
    },
}

impl UpgradeEvent {
    pub fn step(&self) -> UpgradeStep {
        match self {
            Self::CheckingHost => UpgradeStep::CheckHost,
            Self::ResolvingConfig => UpgradeStep::ResolveConfig,
            Self::FetchingRelease { .. } => UpgradeStep::FetchRelease,
            Self::FetchingManifest { .. } => UpgradeStep::FetchManifest,
            Self::ReleaseAvailable { .. } => UpgradeStep::Complete,
            Self::SelectingAsset { .. } => UpgradeStep::SelectAsset,
            Self::DownloadingAsset { .. } => UpgradeStep::DownloadAsset,
            Self::VerifyingChecksum { .. } => UpgradeStep::VerifyChecksum,
            Self::PreparingExecutable { .. } => UpgradeStep::PrepareExecutable,
            Self::ReplacingExecutable { .. } => UpgradeStep::ReplaceExecutable,
            Self::Installed { .. } => UpgradeStep::Complete,
        }
    }
}

pub async fn handle_upgrade(check: bool, rid: Option<RuntimeIdentifier>) -> Result<UpgradeReport> {
    handle_upgrade_with_events(check, rid, |_| {}).await
}

pub async fn handle_upgrade_with_events(
    check: bool,
    rid: Option<RuntimeIdentifier>,
    mut emit: impl FnMut(UpgradeEvent),
) -> Result<UpgradeReport> {
    emit(UpgradeEvent::CheckingHost);
    ensure_supported_host(std::env::current_exe().ok().as_deref())?;
    emit(UpgradeEvent::ResolvingConfig);
    let root = resolve_root(load_user_settings().root.as_deref());
    let workflow = load_workflow_config(&root);
    let options = resolve_updates(&workflow)?;
    emit(UpgradeEvent::FetchingRelease {
        owner: UpgradeOwner::from(options.owner.clone()),
        repository: UpgradeRepositoryName::from(options.repository.clone()),
    });
    let client = reqwest::Client::builder().user_agent("dw/1.0").build()?;
    let release = get_latest_release(&client, &options).await?;
    emit(UpgradeEvent::FetchingManifest {
        asset_name: UpgradeAssetName::from(options.asset_name.clone()),
    });
    let manifest = download_manifest(&client, &release, &options.asset_name).await?;

    if check {
        emit(UpgradeEvent::ReleaseAvailable {
            version: SemanticVersion::from(manifest.version.clone()),
        });
        return Ok(UpgradeReport::Check(upgrade_check_report(
            &release, &manifest,
        )));
    }

    let rid = rid.unwrap_or_else(|| RuntimeIdentifier::from(default_rid()));
    run_upgrade(&client, &manifest, &rid, emit).await
}

pub(crate) fn resolve_updates(workflow: &WorkflowConfig) -> Result<UpdateOptions> {
    let value = workflow.updates.as_ref();
    let owner = string_property(value, "owner").unwrap_or_else(|| DEFAULT_OWNER.into());
    let repository =
        string_property(value, "repository").unwrap_or_else(|| DEFAULT_REPOSITORY.into());
    if owner.trim().is_empty() || repository.trim().is_empty() {
        return Err(anyhow!(
            "Configuration updates.owner / updates.repository manquante dans workflow.json."
        ));
    }
    Ok(UpdateOptions {
        owner,
        repository,
        include_prerelease: value
            .and_then(|value| value.get("includePrerelease"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        asset_name: string_property(value, "assetName")
            .unwrap_or_else(|| DEFAULT_MANIFEST_ASSET.into()),
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
            .ok_or_else(|| anyhow!("Aucune release GitHub trouvée."))
    } else {
        Ok(serde_json::from_str(&body)?)
    }
}

async fn download_manifest(
    client: &reqwest::Client,
    release: &GitHubRelease,
    asset_name: &str,
) -> Result<ReleaseManifest> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(asset_name))
        .ok_or_else(|| anyhow!("Asset release introuvable: {asset_name}"))?;
    let response = client.get(&asset.browser_download_url).send().await?;
    let status = response.status().as_u16();
    let body = response.text().await?;
    if !(200..300).contains(&status) {
        return Err(anyhow!(
            "Téléchargement release.json impossible HTTP {status}: {body}"
        ));
    }
    Ok(serde_json::from_str(&body)?)
}

async fn run_upgrade(
    client: &reqwest::Client,
    manifest: &ReleaseManifest,
    rid: &RuntimeIdentifier,
    mut emit: impl FnMut(UpgradeEvent),
) -> Result<UpgradeReport> {
    emit(UpgradeEvent::SelectingAsset { rid: rid.clone() });
    let asset = manifest
        .assets
        .iter()
        .find(|asset| asset.rid.eq_ignore_ascii_case(rid.as_str()))
        .ok_or_else(|| anyhow!("Aucun asset pour RID {rid}."))?;
    if asset.url.trim().is_empty() {
        return Err(anyhow!(
            "release.json doit contenir assets[].url pour télécharger un asset."
        ));
    }
    let executable_path = std::env::current_exe()?;
    emit(UpgradeEvent::DownloadingAsset {
        file_name: UpgradeFileName::from(asset.file_name.clone()),
    });
    let temp_asset = download_asset(client, asset).await?;
    emit(UpgradeEvent::VerifyingChecksum {
        file_name: UpgradeFileName::from(asset.file_name.clone()),
        expected_sha256: Sha256Digest::from(asset.sha256.clone()),
    });
    let hash_path = temp_asset.clone();
    let hash = tokio::task::spawn_blocking(move || file_sha256(&hash_path)).await??;
    if !hash.eq_ignore_ascii_case(&asset.sha256) {
        let _ = fs::remove_file(&temp_asset);
        return Err(anyhow!(
            "SHA256 invalide pour {}. Attendu {}, obtenu {}.",
            temp_asset.display(),
            asset.sha256,
            hash
        ));
    }
    emit(UpgradeEvent::PreparingExecutable {
        file_name: UpgradeFileName::from(asset.file_name.clone()),
        rid: rid.clone(),
    });
    let asset_file_name = asset.file_name.clone();
    let temp_asset_for_prepare = temp_asset.clone();
    let rid_for_prepare = rid.to_string();
    let replacement = tokio::task::spawn_blocking(move || {
        prepare_replacement_executable(&asset_file_name, &temp_asset_for_prepare, &rid_for_prepare)
    })
    .await??;
    emit(UpgradeEvent::ReplacingExecutable {
        executable_path: ExecutablePath::from(executable_path.display().to_string()),
    });
    let replacement =
        tokio::task::spawn_blocking(move || replace_executable(&executable_path, &replacement))
            .await??;
    emit(UpgradeEvent::Installed {
        version: SemanticVersion::from(manifest.version.clone()),
    });
    Ok(UpgradeReport::Installed(UpgradeInstallReport {
        version: manifest.version.clone(),
        commit: manifest.commit.clone(),
        executable_path: replacement.executable_path.display().to_string(),
        deferred_windows_replacement: replacement.deferred_windows_replacement,
    }))
}

fn upgrade_check_report(release: &GitHubRelease, manifest: &ReleaseManifest) -> UpgradeCheckReport {
    UpgradeCheckReport {
        release_tag: release.tag_name.clone(),
        version: manifest.version.clone(),
        commit: manifest.commit.clone(),
        assets: manifest
            .assets
            .iter()
            .map(|asset| UpgradeAssetSummary {
                rid: asset.rid.clone(),
                file_name: asset.file_name.clone(),
                sha256: asset.sha256.clone(),
            })
            .collect(),
    }
}

async fn download_asset(client: &reqwest::Client, asset: &ReleaseAsset) -> Result<PathBuf> {
    let response = client.get(&asset.url).send().await?;
    let status = response.status().as_u16();
    let body = response.bytes().await?;
    if !(200..300).contains(&status) {
        return Err(anyhow!("Téléchargement upgrade impossible HTTP {status}."));
    }
    let path = std::env::temp_dir().join(format!(
        "dw-upgrade-{}{}",
        std::process::id(),
        extension_suffix(&asset.file_name)
    ));
    fs::write(&path, body)?;
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
        Err(anyhow!("Archive upgrade invalide: dw.exe introuvable."))
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
        Err(anyhow!("Archive upgrade invalide: dw introuvable."))
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
            "Asset upgrade invalide: {display_name} n'est pas un exécutable Windows."
        ));
    }
    Ok(())
}

fn ensure_unix_executable(path: &Path, display_name: &str, rid: &str) -> Result<()> {
    if rid.starts_with("win-") {
        return Err(anyhow!(
            "Asset upgrade invalide: {display_name} n'est pas un exécutable Windows."
        ));
    }
    if !fs::metadata(path)?.is_file() {
        let _ = fs::remove_file(path);
        return Err(anyhow!(
            "Asset upgrade invalide: {display_name} n'est pas un fichier."
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
    let script = script.canonicalize().unwrap_or(script);
    if !script.is_file() {
        return Err(anyhow!(
            "Script de remplacement Windows introuvable après création: {}",
            script.display()
        ));
    }
    let command = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    ProcessCommand::new(&command)
        .arg("/d")
        .arg("/s")
        .arg("/c")
        .arg(windows_replacement_command(&script))
        .current_dir(script.parent().unwrap_or_else(|| Path::new(".")))
        .spawn()
        .map_err(|error| {
            anyhow!(
                "Impossible de lancer le script de remplacement Windows {} via {}: {error}",
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
setlocal
set "NEW={replacement}"
set "TARGET={executable_path}"
set "BACKUP={backup}"
set "PID={pid}"

:wait
tasklist /FI "PID eq %PID%" 2>nul | find "%PID%" >nul
if not errorlevel 1 (
  timeout /t 1 /nobreak >nul
  goto wait
)

if not exist "%NEW%" exit /b 1
if exist "%BACKUP%" del /f /q "%BACKUP%" >nul 2>nul
if exist "%TARGET%" move /Y "%TARGET%" "%BACKUP%" >nul
copy /Y "%NEW%" "%TARGET%" >nul
if errorlevel 1 (
  if exist "%BACKUP%" move /Y "%BACKUP%" "%TARGET%" >nul
  exit /b 1
)
if not exist "%TARGET%" (
  if exist "%BACKUP%" move /Y "%BACKUP%" "%TARGET%" >nul
  exit /b 1
)
del /f /q "%NEW%" >nul 2>nul
del /f /q "%BACKUP%" >nul 2>nul
del /f /q "%~f0" >nul 2>nul
"#
    )
}

pub(crate) fn windows_replacement_command(script: &Path) -> String {
    format!("call \"{}\"", script.display())
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
            "Auto-update indisponible pour une installation Nix. Utiliser un rafraîchissement Nix explicite ou une mise à jour de profil Nix."
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

        assert_eq!(options.owner, "sachahjkl");
        assert_eq!(options.repository, "dw");
        assert!(!options.include_prerelease);
        assert_eq!(options.asset_name, "release.json");
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

        assert_eq!(options.owner, "owner");
        assert_eq!(options.repository, "repo");
        assert!(options.include_prerelease);
        assert_eq!(options.asset_name, "custom.json");
    }

    #[test]
    fn ensure_supported_host_rejects_nix_store_path() {
        let error = ensure_supported_host(Some(Path::new("/nix/store/hash-dw/bin/dw")))
            .expect_err("nix path should fail");

        assert!(error.to_string().contains("Auto-update indisponible"));
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

        assert_eq!(report.release_tag, "v2026.07.03");
        assert_eq!(report.version, "2026.07.03");
        assert_eq!(report.commit, "abcdef0");
        assert_eq!(report.assets.len(), 1);
        assert_eq!(report.assets[0].rid, "linux-x64");
        assert_eq!(report.assets[0].file_name, "dw-linux-x64.tar.gz");
    }

    #[test]
    fn windows_replacement_script_waits_and_restores_backup_on_failure() {
        let script = windows_replacement_script("new.exe", "dw.exe", "dw.exe.bak", 1234);

        assert!(script.contains("tasklist /FI \"PID eq %PID%\""));
        assert!(script.contains("set \"BACKUP=dw.exe.bak\""));
        assert!(script.contains("move /Y \"%TARGET%\" \"%BACKUP%\""));
        assert!(script.contains("copy /Y \"%NEW%\" \"%TARGET%\""));
        assert!(script.contains("move /Y \"%BACKUP%\" \"%TARGET%\""));
        assert!(!script.contains("move /Y \"new.exe\" \"dw.exe\""));
    }

    #[test]
    fn windows_replacement_command_calls_quoted_script_path() {
        let command = windows_replacement_command(Path::new(
            r"C:\Users\me\AppData\Local\Temp\dw upgrade.cmd",
        ));

        assert_eq!(
            command,
            r#"call "C:\Users\me\AppData\Local\Temp\dw upgrade.cmd""#
        );
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

        assert!(error.to_string().contains("dw.exe introuvable"));
        assert!(!path.exists());
    }

    #[test]
    fn prepare_replacement_rejects_zip_when_dw_exe_is_not_windows_executable() {
        let path = create_zip(&[("dw.exe", &[0x50, 0x4b, 0x03, 0x04])]);

        let error = prepare_replacement_executable("dw-win-x64.zip", &path, "win-x64")
            .expect_err("invalid dw.exe should fail");

        assert!(
            error
                .to_string()
                .contains("n'est pas un exécutable Windows")
        );
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
