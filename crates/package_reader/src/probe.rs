//! Common source binding for developer probes; it never qualifies gameplay.
use crate::{IndexInventory, MAX_INDEX_BYTES, read_hed};
use source_identity::SourceManifest;
use std::{
    fs::{self, File, Metadata},
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct ProbeSource {
    root: PathBuf,
    manifest: SourceManifest,
    // Set only by `bind_session`: one complete scan admits a single play
    // session instead of rescanning for every selected asset.
    session: Option<std::sync::OnceLock<()>>,
}

pub struct CheckedIndex {
    pub inventory: IndexInventory,
    pub reader: File,
    package: PathBuf,
    before: Metadata,
}

impl ProbeSource {
    pub fn bind(root: &Path, manifest_path: &Path) -> ProbeResult<Self> {
        let root = fs::canonicalize(root)?;
        let mut bytes = Vec::new();
        File::open(manifest_path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err("source manifest exceeds inspection limit".into());
        }
        let manifest: SourceManifest = serde_json::from_slice(&bytes)?;
        let required = source_identity::required_files();
        if manifest.schema_version != 1
            || manifest.canonical_source_path != root
            || manifest.qualification != "quarantined_unqualified"
            || source_identity::content_identity(&manifest.files) != manifest.content_identity
            || manifest.files.len() != required.len()
            || required.iter().any(|(path, _)| {
                manifest
                    .files
                    .iter()
                    .filter(|f| f.relative_path == *path)
                    .count()
                    != 1
            })
        {
            return Err(
                "source manifest identity/path/schema/candidate layout is inconsistent".into(),
            );
        }
        Ok(Self {
            root,
            manifest,
            session: None,
        })
    }

    /// Binds one interactive session. The first `verify_full_source` still
    /// rehashes every candidate file; later admissions in the same process
    /// reuse that result, and per-index metadata checks still run.
    pub fn bind_session(root: &Path, manifest_path: &Path) -> ProbeResult<Self> {
        let mut source = Self::bind(root, manifest_path)?;
        source.session = Some(std::sync::OnceLock::new());
        Ok(source)
    }

    pub fn content_identity(&self) -> &str {
        &self.manifest.content_identity
    }

    /// Rehash the selected candidate, retaining quarantine. Research viewers
    /// must call this at startup; matching cached metadata is insufficient.
    pub fn verify_full_source(&self) -> ProbeResult<()> {
        if self.session.as_ref().is_some_and(|s| s.get().is_some()) {
            return Ok(());
        }
        let fresh = source_identity::scan(&self.root)?;
        if fresh.content_identity != self.manifest.content_identity
            || fresh.files.iter().any(|observed| {
                self.manifest
                    .files
                    .iter()
                    .find(|expected| expected.relative_path == observed.relative_path)
                    .is_none_or(|expected| {
                        observed.sha256 != expected.sha256
                            || observed.size != expected.size
                            || observed.modified_unix_nanos != expected.modified_unix_nanos
                    })
            })
        {
            return Err(
                "selected source differs from the bound manifest; full scan rejected".into(),
            );
        }
        if let Some(session) = &self.session {
            let _ = session.set(());
        }
        Ok(())
    }

    pub fn output_path(&self, path: &Path) -> std::io::Result<PathBuf> {
        source_identity::safe_output(&self.root, path)
    }

    pub fn open_index(&self, relative: &str) -> ProbeResult<CheckedIndex> {
        if !source_identity::required_files()
            .iter()
            .any(|(name, _)| name == relative && name.ends_with(".hed"))
        {
            return Err("index is outside the inspected KH2 candidate set".into());
        }
        let expected = self
            .manifest
            .files
            .iter()
            .find(|f| f.relative_path == relative)
            .ok_or("missing bound index")?;
        let hed = fs::canonicalize(self.root.join(relative))?;
        if !hed.starts_with(&self.root) {
            return Err("index escapes source".into());
        }
        if expected.size > MAX_INDEX_BYTES as u64
            || fs::metadata(&hed)?.len() > MAX_INDEX_BYTES as u64
        {
            return Err("index exceeds inspection limit before hashing".into());
        }
        let observed = source_identity::inspect_file(&self.root, relative, "index_check")?;
        if observed.sha256 != expected.sha256
            || observed.size != expected.size
            || observed.modified_unix_nanos != expected.modified_unix_nanos
        {
            return Err("index differs from bound source manifest".into());
        }
        let package_relative = relative
            .strip_suffix(".hed")
            .ok_or("index extension")?
            .to_owned()
            + ".pkg";
        // Pair by the manifest's logical path even when the HED is an in-root link.
        let package = fs::canonicalize(self.root.join(&package_relative))?;
        if !package.starts_with(&self.root) {
            return Err("package escapes source".into());
        }
        let expected = self
            .manifest
            .files
            .iter()
            .find(|f| f.relative_path == package_relative)
            .ok_or("missing bound package")?;
        let reader = File::open(&package)?;
        let before = reader.metadata()?;
        if !before.is_file()
            || before.len() != expected.size
            || before.modified()?.duration_since(UNIX_EPOCH)?.as_nanos()
                != expected.modified_unix_nanos
        {
            return Err("package metadata differs from source manifest; rescan fully".into());
        }
        let mut bytes = Vec::new();
        File::open(hed)?
            .take((MAX_INDEX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let inventory = read_hed(&bytes, before.len())?;
        Ok(CheckedIndex {
            inventory,
            reader,
            package,
            before,
        })
    }
}

impl CheckedIndex {
    pub fn verify_unchanged(&self) -> ProbeResult<()> {
        for after in [self.reader.metadata()?, fs::metadata(&self.package)?] {
            if after.len() != self.before.len() || after.modified()? != self.before.modified()? {
                return Err("package changed during source inspection".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
        manifest: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "kh2-probe-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let root = base.join("source");
            fs::create_dir_all(root.join("Image/dt")).unwrap();
            for (relative, _) in source_identity::required_files() {
                let data = if relative.ends_with(".hed") {
                    let mut record = vec![0; 24];
                    record.extend(16i32.to_le_bytes());
                    record.extend(0i32.to_le_bytes());
                    record
                } else {
                    vec![0; 16]
                };
                fs::write(root.join(relative), data).unwrap();
            }
            let manifest = base.join("manifest.json");
            fs::write(
                &manifest,
                serde_json::to_vec(&source_identity::scan(&root).unwrap()).unwrap(),
            )
            .unwrap();
            Self {
                base,
                root,
                manifest,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.base).unwrap();
        }
    }

    #[test]
    fn binding_rejects_changed_indexes_packages_and_unlisted_inputs() {
        let fixture = Fixture::new();
        let source = ProbeSource::bind(&fixture.root, &fixture.manifest).unwrap();
        let checked = source.open_index("Image/dt/kh2_first.hed").unwrap();
        assert_eq!(checked.inventory.record_count, 1);
        checked.verify_unchanged().unwrap();
        assert!(source.open_index("../outside.hed").is_err());
        fs::write(fixture.root.join("Image/dt/kh2_first.pkg"), [0; 17]).unwrap();
        assert!(checked.verify_unchanged().is_err());
        assert!(source.open_index("Image/dt/kh2_first.hed").is_err());
        let hed = fixture.root.join("Image/dt/kh2_second.hed");
        let mut bytes = fs::read(&hed).unwrap();
        bytes[0] = 1;
        fs::write(hed, bytes).unwrap();
        assert!(source.open_index("Image/dt/kh2_second.hed").is_err());
        File::create(fixture.root.join("Image/dt/kh2_third.hed"))
            .unwrap()
            .set_len(MAX_INDEX_BYTES as u64 + 1)
            .unwrap();
        assert!(
            source
                .open_index("Image/dt/kh2_third.hed")
                .err()
                .unwrap()
                .to_string()
                .contains("before hashing")
        );
    }

    #[test]
    fn binding_rejects_incomplete_recomputed_manifest_and_wrong_root() {
        let fixture = Fixture::new();
        assert!(ProbeSource::bind(&fixture.base, &fixture.manifest).is_err());
        let mut manifest: SourceManifest =
            serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
        manifest.files.pop();
        manifest.content_identity = source_identity::content_identity(&manifest.files);
        fs::write(&fixture.manifest, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(ProbeSource::bind(&fixture.root, &fixture.manifest).is_err());
    }

    #[test]
    fn full_validation_checks_the_executable_as_bytes_without_loading_it() {
        let fixture = Fixture::new();
        let source = ProbeSource::bind(&fixture.root, &fixture.manifest).unwrap();
        source.verify_full_source().unwrap();
        fs::write(fixture.root.join(source_identity::KH2_EXECUTABLE), [1; 16]).unwrap();
        assert!(source.verify_full_source().is_err());
    }

    #[test]
    fn model_inspection_rejects_stale_unselected_executable_before_decoding() {
        let fixture = Fixture::new();
        let mut plain_empty_header = [0u8; 16];
        plain_empty_header[8..12].copy_from_slice(&(-2i32).to_le_bytes());
        fs::write(
            fixture.root.join("Image/dt/kh2_first.pkg"),
            plain_empty_header,
        )
        .unwrap();
        fs::write(
            &fixture.manifest,
            serde_json::to_vec(&source_identity::scan(&fixture.root).unwrap()).unwrap(),
        )
        .unwrap();
        let source = ProbeSource::bind(&fixture.root, &fixture.manifest).unwrap();
        let path = fixture.base.join("selection.json");
        let mut selection = serde_json::json!({"schema_version":1,
            "source_content_identity":source.content_identity(),"inspect_model_geometry":true,
            "model_pose_profiles":["openkh_legacy_bind_fma_v1"],
            "selections":[{"index":"Image/dt/kh2_first.hed","ordinal":0,"name_hash":vec![0u8;16]}]});
        fs::write(&path, serde_json::to_vec(&selection).unwrap()).unwrap();
        let decoding_error = crate::model_inspection::read_selected_models(&source, &path)
            .err()
            .unwrap()
            .to_string();
        assert!(decoding_error.contains("invalid BAR"), "{decoding_error}");
        fs::write(fixture.root.join(source_identity::KH2_EXECUTABLE), [1; 16]).unwrap();
        assert!(
            crate::model_inspection::read_selected_models(&source, &path)
                .err()
                .unwrap()
                .to_string()
                .contains("full scan rejected")
        );
        selection["model_pose_profiles"] = serde_json::json!([]);
        fs::write(&path, serde_json::to_vec(&selection).unwrap()).unwrap();
        assert!(
            crate::model_inspection::read_selected_models(&source, &path)
                .err()
                .unwrap()
                .to_string()
                .contains("explicit profiles invalid")
        );
    }
}
