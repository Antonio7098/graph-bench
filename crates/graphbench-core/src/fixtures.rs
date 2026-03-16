use crate::artifacts::FixtureManifest;
use crate::error::{AppError, ErrorCode, ErrorContext};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureResolution {
    pub manifest_path: PathBuf,
    pub repository_root: PathBuf,
    pub snapshot_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct FixtureRepository;

impl FixtureRepository {
    pub fn load(
        &self,
        manifest_path: impl AsRef<Path>,
    ) -> Result<(FixtureManifest, FixtureResolution), AppError> {
        let manifest_path = manifest_path.as_ref();
        let manifest = load_fixture_manifest(manifest_path)?;
        let resolution = resolve_fixture(manifest_path, &manifest)?;
        validate_snapshot_identity(&resolution, &manifest)?;
        Ok((manifest, resolution))
    }
}

pub fn load_fixture_manifest(path: impl AsRef<Path>) -> Result<FixtureManifest, AppError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| {
        AppError::with_source(
            ErrorCode::FixtureManifestInvalid,
            format!("failed to read fixture manifest at {}", path.display()),
            ErrorContext {
                component: "fixtures",
                operation: "read_fixture_manifest",
            },
            source,
        )
    })?;

    let manifest: FixtureManifest = serde_json::from_str(&raw).map_err(|source| {
        AppError::with_source(
            ErrorCode::FixtureManifestInvalid,
            format!("failed to parse fixture manifest at {}", path.display()),
            ErrorContext {
                component: "fixtures",
                operation: "parse_fixture_manifest",
            },
            source,
        )
    })?;

    manifest.validate()?;
    Ok(manifest)
}

pub fn load_fixture_manifests(
    root: impl AsRef<Path>,
) -> Result<Vec<(FixtureManifest, FixtureResolution)>, AppError> {
    let mut manifests = Vec::new();
    for path in collect_json_files(root.as_ref())? {
        if path.file_name().and_then(std::ffi::OsStr::to_str) == Some("fixture.json") {
            manifests.push(FixtureRepository.load(path)?);
        }
    }

    manifests.sort_by(|left, right| left.0.fixture_id.cmp(&right.0.fixture_id));
    Ok(manifests)
}

fn resolve_fixture(
    manifest_path: &Path,
    manifest: &FixtureManifest,
) -> Result<FixtureResolution, AppError> {
    let manifest_dir = manifest_path.parent().ok_or_else(|| {
        AppError::new(
            ErrorCode::FixtureManifestInvalid,
            "fixture manifest must live in a directory",
            ErrorContext {
                component: "fixtures",
                operation: "resolve_fixture",
            },
        )
    })?;

    let repository_root = manifest_dir
        .join(&manifest.repository.source)
        .canonicalize()
        .map_err(|source| {
            AppError::with_source(
                ErrorCode::FixtureManifestInvalid,
                format!(
                    "fixture repository source '{}' could not be resolved",
                    manifest.repository.source
                ),
                ErrorContext {
                    component: "fixtures",
                    operation: "resolve_fixture_repository",
                },
                source,
            )
        })?;

    let snapshot_path = manifest_dir
        .join(&manifest.graph.snapshot_ref)
        .canonicalize()
        .map_err(|source| {
            AppError::with_source(
                ErrorCode::GraphSnapshotMissing,
                format!(
                    "graph snapshot ref '{}' could not be resolved",
                    manifest.graph.snapshot_ref
                ),
                ErrorContext {
                    component: "fixtures",
                    operation: "resolve_graph_snapshot",
                },
                source,
            )
        })?;

    Ok(FixtureResolution {
        manifest_path: manifest_path.to_path_buf(),
        repository_root,
        snapshot_path,
    })
}

fn validate_snapshot_identity(
    resolution: &FixtureResolution,
    manifest: &FixtureManifest,
) -> Result<(), AppError> {
    let bytes = fs::read(&resolution.snapshot_path).map_err(|source| {
        AppError::with_source(
            ErrorCode::GraphSnapshotMissing,
            format!(
                "failed to read graph snapshot at {}",
                resolution.snapshot_path.display()
            ),
            ErrorContext {
                component: "fixtures",
                operation: "read_graph_snapshot",
            },
            source,
        )
    })?;

    let actual_snapshot_id = sha256_of(&bytes);
    if actual_snapshot_id != manifest.graph.snapshot_id {
        return Err(AppError::new(
            ErrorCode::FixtureManifestInvalid,
            format!(
                "graph snapshot identity drift: expected {}, got {}",
                manifest.graph.snapshot_id, actual_snapshot_id
            ),
            ErrorContext {
                component: "fixtures",
                operation: "validate_snapshot_identity",
            },
        ));
    }

    Ok(())
}

fn collect_json_files(root: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    visit(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), AppError> {
    for entry in fs::read_dir(root).map_err(|source| {
        AppError::with_source(
            ErrorCode::FixtureManifestInvalid,
            format!("failed to read directory {}", root.display()),
            ErrorContext {
                component: "fixtures",
                operation: "read_directory",
            },
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            AppError::with_source(
                ErrorCode::FixtureManifestInvalid,
                format!("failed to read directory entry under {}", root.display()),
                ErrorContext {
                    component: "fixtures",
                    operation: "read_directory_entry",
                },
                source,
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            visit(&path, files)?;
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("json") {
            files.push(path);
        }
    }

    Ok(())
}

pub(crate) fn sha256_of(bytes: &[u8]) -> String {
    let mut data = bytes.to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while (data.len() % 64) != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0: u32 = 0x6a09e667;
    let mut h1: u32 = 0xbb67ae85;
    let mut h2: u32 = 0x3c6ef372;
    let mut h3: u32 = 0xa54ff53a;
    let mut h4: u32 = 0x510e527f;
    let mut h5: u32 = 0x9b05688c;
    let mut h6: u32 = 0x1f83d9ab;
    let mut h7: u32 = 0x5be0cd19;

    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    for chunk in data.chunks(64) {
        let mut w = [0_u32; 64];
        for (index, word) in chunk.chunks(4).take(16).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        let mut f = h5;
        let mut g = h6;
        let mut h = h7;

        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
        h5 = h5.wrapping_add(f);
        h6 = h6.wrapping_add(g);
        h7 = h7.wrapping_add(h);
    }

    format!("sha256:{h0:08x}{h1:08x}{h2:08x}{h3:08x}{h4:08x}{h5:08x}{h6:08x}{h7:08x}")
}

#[cfg(test)]
mod tests {
    use super::{FixtureRepository, load_fixture_manifests, sha256_of};
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sha256_matches_known_value() {
        assert_eq!(
            sha256_of(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn internal_fixture_loads() {
        let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/graphbench-internal/fixture.json");
        let repository = FixtureRepository;
        let (manifest, resolution) = repository.load(manifest_path).expect("fixture should load");
        assert_eq!(manifest.fixture_id, "graphbench.internal");
        assert!(resolution.snapshot_path.ends_with("graph.snapshot.json"));
    }

    #[test]
    fn fixture_repository_loads_all_manifests() {
        let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let manifests = load_fixture_manifests(fixtures_root).expect("fixtures should load");
        assert_eq!(manifests.len(), 1);
    }

    #[test]
    fn drifting_snapshot_identity_fails_fast() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!("graphbench-fixture-drift-{unique}"));
        fs::create_dir_all(&temp_root).expect("temp dir should be created");

        let snapshot_path = temp_root.join("graph.snapshot.json");
        fs::write(&snapshot_path, "{\"fixture_id\":\"drift\"}")
            .expect("snapshot should be written");

        let manifest_path = temp_root.join("fixture.json");
        fs::write(
            &manifest_path,
            format!(
                concat!(
                    "{{",
                    "\"fixture_id\":\"graphbench.internal\",",
                    "\"schema_version\":1,",
                    "\"repository\":{{\"source\":\".\",\"commit_sha\":\"1111111111111111111111111111111111111111\",\"mirror_policy\":\"workspace\"}},",
                    "\"graph\":{{\"snapshot_id\":\"{}\",\"snapshot_format\":\"json\",\"snapshot_ref\":\"graph.snapshot.json\"}},",
                    "\"languages\":[\"rust\"],",
                    "\"metadata\":{{\"title\":\"Drift Fixture\",\"notes\":\"\"}}",
                    "}}"
                ),
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
        )
        .expect("manifest should be written");

        let repository = FixtureRepository;
        let result = repository.load(&manifest_path);
        assert!(result.is_err());

        fs::remove_dir_all(temp_root).expect("temp dir should be removed");
    }
}
