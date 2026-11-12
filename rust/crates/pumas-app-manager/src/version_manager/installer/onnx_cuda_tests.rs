//! The CUDA libraries of ONNX Runtime CUDA builds.
//!
//! A loopback server stands in for PyPI, Microsoft's ONNX Runtime package
//! feed, the GitHub release API and their downloads. Its wheels are small
//! fixture zips laid out like NVIDIA's: no real wheel or ONNX Runtime archive
//! is downloaded.

use super::onnx_cuda::{self, CudaWheelSources};
use super::onnx_cuda_availability::CudaAvailability;
use super::python_index::WheelPlatform;
use super::*;
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt as _};

const RELEASE: &str = "1.24.2";

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Bytes that do not compress, so a wheel is as large as its payload.
fn noise(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    (0..length)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u8
        })
        .collect()
}

#[derive(Clone)]
struct Route {
    body: Vec<u8>,
    content_type: &'static str,
}

#[derive(Default)]
struct ServerState {
    routes: HashMap<String, Route>,
    /// Paths whose next full response stops halfway through the body.
    truncate_once: HashSet<String>,
    /// Every request: its path and any `Range` header.
    log: Vec<(String, Option<String>)>,
    /// Paths answered only once they are released.
    stalled: HashSet<String>,
    /// Paths answered with a server error.
    failing: HashSet<String>,
}

/// A loopback HTTP origin with byte-range support.
struct FixtureServer {
    base_url: String,
    state: Arc<StdMutex<ServerState>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FixtureServer {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(StdMutex::new(ServerState::default()));
        let shared = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let shared = shared.clone();
                tokio::spawn(async move { serve(stream, shared).await });
            }
        });
        Self {
            base_url,
            state,
            task,
        }
    }

    fn route(&self, path: &str, body: Vec<u8>, content_type: &'static str) {
        self.state
            .lock()
            .unwrap()
            .routes
            .insert(path.to_owned(), Route { body, content_type });
    }

    fn stall(&self, path: &str) {
        self.state.lock().unwrap().stalled.insert(path.to_owned());
    }

    fn fail(&self, path: &str) {
        self.state.lock().unwrap().failing.insert(path.to_owned());
    }

    fn release(&self, path: &str) {
        self.state.lock().unwrap().stalled.remove(path);
    }

    fn unroute(&self, path: &str) {
        self.state.lock().unwrap().routes.remove(path);
    }

    fn truncate_once(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .truncate_once
            .insert(path.to_owned());
    }

    fn requests(&self, path: &str) -> Vec<Option<String>> {
        self.state
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|(requested, _)| requested == path)
            .map(|(_, range)| range.clone())
            .collect()
    }

    fn requested_paths(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .log
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    }
}

async fn serve(mut stream: tokio::net::TcpStream, state: Arc<StdMutex<ServerState>>) {
    let mut request = Vec::new();
    let mut buffer = [0; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
        if request.len() > 64 * 1024 {
            return;
        }
    }
    let text = String::from_utf8_lossy(&request).into_owned();
    let mut lines = text.lines();
    let request_line = lines.next().unwrap_or_default().to_owned();
    let head_only = request_line.starts_with("HEAD ");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let range = lines.take_while(|line| !line.is_empty()).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("range")
            .then(|| value.trim().to_owned())
    });
    let mut stalled = {
        let mut state = state.lock().unwrap();
        // A HEAD request is logged apart from downloads of the same path.
        let logged = if head_only {
            format!("HEAD {path}")
        } else {
            path.clone()
        };
        state.log.push((logged, range.clone()));
        state.stalled.contains(&path)
    };
    // A stalled path answers once it is released.
    while stalled {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        stalled = state.lock().unwrap().stalled.contains(&path);
    }
    let (route, truncate) = {
        let mut state = state.lock().unwrap();
        let route = state.routes.get(&path).cloned();
        let truncate = range.is_none() && state.truncate_once.remove(&path);
        (route, truncate)
    };
    if state.lock().unwrap().failing.contains(&path) {
        let _ = stream
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await;
        return;
    }
    let Some(route) = route else {
        let _ = stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return;
    };
    let total = route.body.len();
    let start = range
        .as_deref()
        .and_then(|range| range.strip_prefix("bytes="))
        .and_then(|range| range.strip_suffix('-'))
        .and_then(|start| start.parse::<usize>().ok());
    let (status, body, extra) = match start {
        Some(start) if start < total => (
            "206 Partial Content",
            &route.body[start..],
            format!("Content-Range: bytes {start}-{}/{total}\r\n", total - 1),
        ),
        _ => ("200 OK", &route.body[..], String::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
        route.content_type,
        body.len()
    );
    if stream.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    let sent = if head_only {
        0
    } else if truncate {
        body.len() / 2
    } else {
        body.len()
    };
    let _ = stream.write_all(&body[..sent]).await;
    let _ = stream.flush().await;
}

const SIMPLE_JSON: &str = "application/vnd.pypi.simple.v1+json";

/// One file an index lists.
struct IndexEntry {
    filename: String,
    bytes: Vec<u8>,
    metadata: Option<String>,
    yanked: bool,
    sha256: bool,
}

impl IndexEntry {
    fn wheel(filename: &str, bytes: Vec<u8>, requires: &[&str]) -> Self {
        let mut metadata = String::from("Metadata-Version: 2.1\nName: fixture\nVersion: 0\n");
        for requirement in requires {
            metadata.push_str(&format!("Requires-Dist: {requirement}\n"));
        }
        Self {
            filename: filename.to_owned(),
            bytes,
            metadata: Some(metadata),
            yanked: false,
            sha256: true,
        }
    }
}

/// Serve `project` on the index at `prefix` (`/pypi/simple`).
fn publish(server: &FixtureServer, prefix: &str, project: &str, entries: Vec<IndexEntry>) {
    let files: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            let path = format!("/files{prefix}/{}", entry.filename);
            server.route(&path, entry.bytes.clone(), "application/octet-stream");
            let mut file = serde_json::json!({
                "filename": entry.filename,
                "url": path,
                "size": entry.bytes.len(),
                "yanked": entry.yanked,
                "hashes": if entry.sha256 {
                    serde_json::json!({ "sha256": sha256_hex(&entry.bytes) })
                } else {
                    serde_json::json!({})
                },
            });
            if let Some(metadata) = &entry.metadata {
                server.route(
                    &format!("{path}.metadata"),
                    metadata.as_bytes().to_vec(),
                    "text/plain",
                );
                file["core-metadata"] =
                    serde_json::json!({ "sha256": sha256_hex(metadata.as_bytes()) });
            }
            file
        })
        .collect();
    server.route(
        &format!("{prefix}/{project}/"),
        serde_json::to_vec(&serde_json::json!({
            "meta": { "api-version": "1.1" },
            "name": project,
            "files": files,
        }))
        .unwrap(),
        SIMPLE_JSON,
    );
}

fn platform_tag(platform: WheelPlatform) -> &'static str {
    match platform {
        WheelPlatform::LinuxX64 => "manylinux_2_27_x86_64",
        WheelPlatform::WindowsX64 => "win_amd64",
    }
}

/// A wheel laid out like NVIDIA's: `nvidia/<component>/lib/<library>` on
/// Linux and `nvidia/<component>/bin/<library>` on Windows, with a static
/// library and header beside it and a license in its dist-info.
fn nvidia_wheel(
    platform: WheelPlatform,
    distribution: &str,
    version: &str,
    component: &str,
    libraries: &[(&str, &str)],
    payload: usize,
) -> Vec<u8> {
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
    let mut writer = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (seed, (linux, windows)) in libraries.iter().enumerate() {
        let path = match platform {
            WheelPlatform::LinuxX64 => format!("nvidia/{component}/lib/{linux}"),
            WheelPlatform::WindowsX64 => format!("nvidia/{component}/bin/{windows}"),
        };
        writer.start_file(path, stored).unwrap();
        writer
            .write_all(&noise(payload, seed as u64 + distribution.len() as u64))
            .unwrap();
    }
    writer
        .start_file(format!("nvidia/{component}/lib/static_part.a"), stored)
        .unwrap();
    writer.write_all(b"static archive").unwrap();
    writer
        .start_file(format!("nvidia/{component}/include/api.h"), stored)
        .unwrap();
    writer.write_all(b"/* header */").unwrap();
    writer.start_file("nvidia/__init__.py", stored).unwrap();
    let dist_info = format!("{distribution}-{version}.dist-info");
    writer
        .start_file(format!("{dist_info}/licenses/License.txt"), stored)
        .unwrap();
    writer.write_all(b"NVIDIA license fixture").unwrap();
    writer
        .start_file(format!("{dist_info}/METADATA"), stored)
        .unwrap();
    writer.write_all(b"Metadata-Version: 2.1").unwrap();
    writer.finish().unwrap().into_inner()
}

struct NvidiaPackage {
    project: &'static str,
    distribution: &'static str,
    version: &'static str,
    component: &'static str,
    libraries: &'static [(&'static str, &'static str)],
    requires: &'static [&'static str],
}

/// The CUDA 12 closure a current `onnxruntime-gpu` resolves to.
const CUDA12_PACKAGES: &[NvidiaPackage] = &[
    NvidiaPackage {
        project: "nvidia-cuda-runtime-cu12",
        distribution: "nvidia_cuda_runtime_cu12",
        version: "12.9.79",
        component: "cuda_runtime",
        libraries: &[("libcudart.so.12", "cudart64_12.dll")],
        requires: &[],
    },
    NvidiaPackage {
        project: "nvidia-cufft-cu12",
        distribution: "nvidia_cufft_cu12",
        version: "11.4.1.4",
        component: "cufft",
        libraries: &[("libcufft.so.11", "cufft64_11.dll")],
        requires: &["nvidia-nvjitlink-cu12"],
    },
    NvidiaPackage {
        project: "nvidia-nvjitlink-cu12",
        distribution: "nvidia_nvjitlink_cu12",
        version: "12.9.86",
        component: "nvjitlink",
        libraries: &[("libnvJitLink.so.12", "nvJitLink_120_0.dll")],
        requires: &[],
    },
    NvidiaPackage {
        project: "nvidia-cudnn-cu12",
        distribution: "nvidia_cudnn_cu12",
        version: "9.28.1.4",
        component: "cudnn",
        libraries: &[
            ("libcudnn.so.9", "cudnn64_9.dll"),
            ("libcudnn_ops.so.9", "cudnn_ops64_9.dll"),
        ],
        // A Python dependency no library needs is not followed.
        requires: &[
            "nvidia-cublas-cu12",
            "setuptools; python_version >= \"3.12\"",
        ],
    },
    NvidiaPackage {
        project: "nvidia-cublas-cu12",
        distribution: "nvidia_cublas_cu12",
        version: "12.9.3.13",
        component: "cublas",
        libraries: &[
            ("libcublas.so.12", "cublas64_12.dll"),
            ("libcublasLt.so.12", "cublasLt64_12.dll"),
        ],
        requires: &["nvidia-cuda-nvrtc-cu12"],
    },
    NvidiaPackage {
        project: "nvidia-cuda-nvrtc-cu12",
        distribution: "nvidia_cuda_nvrtc_cu12",
        version: "12.9.86",
        component: "cuda_nvrtc",
        libraries: &[("libnvrtc.so.12", "nvrtc64_120_0.dll")],
        requires: &[],
    },
];

fn wheel_filename(platform: WheelPlatform, distribution: &str, version: &str) -> String {
    format!(
        "{distribution}-{version}-py3-none-{}.whl",
        platform_tag(platform)
    )
}

fn package_entry(platform: WheelPlatform, package: &NvidiaPackage, payload: usize) -> IndexEntry {
    IndexEntry::wheel(
        &wheel_filename(platform, package.distribution, package.version),
        nvidia_wheel(
            platform,
            package.distribution,
            package.version,
            package.component,
            package.libraries,
            payload,
        ),
        package.requires,
    )
}

const CUDA12_EXTRAS: &[&str] = &[
    "flatbuffers",
    "numpy>=1.21.6",
    "nvidia-cuda-runtime-cu12~=12.0; extra == \"cuda\"",
    "nvidia-cufft-cu12~=11.0; extra == \"cuda\"",
    "nvidia-cudnn-cu12~=9.0; extra == \"cudnn\"",
    "sympy; extra == \"symbolic\"",
];

const CUDA13_EXTRAS: &[&str] = &[
    "numpy>=1.21.6",
    "nvidia-cuda-runtime~=13.0; extra == \"cuda\"",
    "nvidia-cudnn-cu13~=9.0; extra == \"cudnn\"",
];

fn publish_onnxruntime_gpu(
    server: &FixtureServer,
    prefix: &str,
    platform: WheelPlatform,
    requires: &[&str],
) {
    let filename = format!(
        "onnxruntime_gpu-{RELEASE}-cp312-cp312-{}.whl",
        platform_tag(platform)
    );
    publish(
        server,
        prefix,
        "onnxruntime-gpu",
        vec![
            IndexEntry::wheel(&filename, b"never downloaded".to_vec(), requires),
            // Another release on the same page.
            IndexEntry::wheel(
                &format!(
                    "onnxruntime_gpu-1.23.2-cp312-cp312-{}.whl",
                    platform_tag(platform)
                ),
                b"older".to_vec(),
                CUDA13_EXTRAS,
            ),
        ],
    );
}

/// Publish the CUDA 12 closure on `/pypi/simple`, with releases that must not
/// be chosen beside the right ones.
fn publish_cuda12_closure(server: &FixtureServer, platform: WheelPlatform, payload: usize) {
    for package in CUDA12_PACKAGES {
        let mut entries = vec![package_entry(platform, package, payload)];
        match package.project {
            "nvidia-cuda-runtime-cu12" => {
                // Outside `~=12.0`, a pre-release and a yanked release.
                for (version, yanked) in
                    [("13.0.88", false), ("12.9.80rc1", false), ("12.9.81", true)]
                {
                    let mut entry = IndexEntry::wheel(
                        &wheel_filename(platform, package.distribution, version),
                        b"not chosen".to_vec(),
                        &[],
                    );
                    entry.yanked = yanked;
                    entries.push(entry);
                }
                entries.push(IndexEntry::wheel(
                    &wheel_filename(platform, package.distribution, "12.8.90"),
                    b"older".to_vec(),
                    &[],
                ));
            }
            "nvidia-nvjitlink-cu12" => {
                // Newer, but published for another platform only.
                entries.push(IndexEntry::wheel(
                    &format!(
                        "{}-12.9.90-py3-none-manylinux_2_27_aarch64.whl",
                        package.distribution
                    ),
                    b"other platform".to_vec(),
                    &[],
                ));
            }
            _ => {}
        }
        publish(server, "/pypi/simple", package.project, entries);
    }
}

fn sources(server: &FixtureServer) -> CudaWheelSources {
    CudaWheelSources {
        requirement_indexes: vec![
            format!("{}/pypi/simple/", server.base_url),
            format!("{}/feed{{major}}/simple/", server.base_url),
        ],
        wheel_index: format!("{}/pypi/simple/", server.base_url),
        client: Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        cache: Arc::default(),
    }
}

fn release() -> semver::Version {
    semver::Version::parse(RELEASE).unwrap()
}

#[tokio::test]
async fn resolution_follows_published_requirements_to_current_compatible_releases() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA12_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);

    let selection = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap();

    let chosen: Vec<_> = selection
        .wheels
        .iter()
        .map(|wheel| (wheel.project.as_str(), wheel.version.as_str()))
        .collect();
    assert_eq!(
        chosen,
        [
            ("nvidia-cublas-cu12", "12.9.3.13"),
            ("nvidia-cuda-nvrtc-cu12", "12.9.86"),
            ("nvidia-cuda-runtime-cu12", "12.9.79"),
            ("nvidia-cudnn-cu12", "9.28.1.4"),
            ("nvidia-cufft-cu12", "11.4.1.4"),
            ("nvidia-nvjitlink-cu12", "12.9.86"),
        ]
    );
    for wheel in &selection.wheels {
        assert_eq!(wheel.sha256.len(), 64);
        assert!(wheel.url.starts_with(&server.base_url), "{}", wheel.url);
    }
    assert_eq!(selection.cuda_major, 12);
    assert!(selection.requirements.index.contains("/pypi/simple"));
    assert_eq!(selection.requirements.version, RELEASE);
    assert_eq!(selection.requirements.requires.len(), 3);
    let paths = server.requested_paths();
    assert!(
        !paths
            .iter()
            .any(|path| path.contains("onnxruntime_gpu") && !path.ends_with(".metadata")),
        "only metadata of onnxruntime-gpu is read: {paths:?}"
    );
    assert!(
        !paths.iter().any(|path| path.contains("setuptools")),
        "non-NVIDIA requirements are not followed: {paths:?}"
    );
    assert!(
        !paths.iter().any(|path| path.ends_with(".whl")),
        "resolution downloads no wheel: {paths:?}"
    );
}

#[tokio::test]
async fn a_cuda_major_published_on_the_onnx_runtime_feed_is_read_there() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::WindowsX64;
    // PyPI carries this release's CUDA 13 build; the CUDA 12 build is on
    // Microsoft's CUDA 12 feed.
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA13_EXTRAS);
    publish_onnxruntime_gpu(&server, "/feed12/simple", platform, CUDA12_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);

    let selection = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap();
    assert!(
        selection.requirements.index.contains("/feed12/simple"),
        "{:?}",
        selection.requirements
    );
    assert!(selection
        .wheels
        .iter()
        .all(|wheel| wheel.filename.ends_with("-win_amd64.whl")));
    assert!(selection
        .wheels
        .iter()
        .any(|wheel| wheel.project == "nvidia-cuda-runtime-cu12"));
}

#[tokio::test]
async fn a_build_without_published_metadata_is_refused_with_the_reason() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA13_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);

    let refused = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        refused.contains("has no published CUDA 12 package metadata"),
        "{refused}"
    );
    assert!(refused.contains("is the CUDA 13 build"), "{refused}");
    assert!(
        refused.contains("/feed12/simple/ has no onnxruntime-gpu"),
        "{refused}"
    );
    assert!(refused.contains("does not guess"), "{refused}");
    assert!(
        !server
            .requested_paths()
            .iter()
            .any(|path| path.contains("nvidia")),
        "nothing is resolved without the build's requirements"
    );
}

#[tokio::test]
async fn a_wheel_without_a_published_sha256_is_refused() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA12_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);
    let mut unverifiable = package_entry(platform, &CUDA12_PACKAGES[0], 64);
    unverifiable.sha256 = false;
    publish(
        &server,
        "/pypi/simple",
        CUDA12_PACKAGES[0].project,
        vec![unverifiable],
    );

    let refused = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(refused.contains("publishes no SHA-256"), "{refused}");
    assert!(
        refused.contains("nvidia_cuda_runtime_cu12-12.9.79"),
        "{refused}"
    );
}

#[tokio::test]
async fn an_older_release_is_tried_when_the_newest_needs_what_does_not_exist() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA12_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);
    // The newest cuDNN needs a cuBLAS release that has no wheel for the host.
    let cudnn = CUDA12_PACKAGES
        .iter()
        .find(|package| package.project == "nvidia-cudnn-cu12")
        .unwrap();
    publish(
        &server,
        "/pypi/simple",
        cudnn.project,
        vec![
            package_entry(platform, cudnn, 64),
            IndexEntry::wheel(
                &wheel_filename(platform, cudnn.distribution, "9.29.0.1"),
                b"needs a missing cuBLAS".to_vec(),
                &["nvidia-cublas-cu12>=12.10"],
            ),
        ],
    );

    let selection = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap();
    let chosen = |project: &str| {
        selection
            .wheels
            .iter()
            .find(|wheel| wheel.project == project)
            .map(|wheel| wheel.version.clone())
    };
    assert_eq!(chosen("nvidia-cudnn-cu12").as_deref(), Some("9.28.1.4"));
    assert_eq!(chosen("nvidia-cublas-cu12").as_deref(), Some("12.9.3.13"));
}

#[tokio::test]
async fn a_resolved_runtime_of_another_cuda_major_is_refused() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    // Metadata that names CUDA 12 but allows a CUDA 13 runtime release.
    publish_onnxruntime_gpu(
        &server,
        "/pypi/simple",
        platform,
        &["nvidia-cuda-runtime-cu12>=12.0; extra == \"cuda\""],
    );
    publish(
        &server,
        "/pypi/simple",
        "nvidia-cuda-runtime-cu12",
        vec![IndexEntry::wheel(
            &wheel_filename(platform, "nvidia_cuda_runtime_cu12", "13.0.88"),
            b"mislabelled".to_vec(),
            &[],
        )],
    );
    let refused = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(refused.contains("is not a CUDA 12 runtime"), "{refused}");
}

#[tokio::test]
async fn requirements_no_published_wheel_satisfies_are_refused_with_the_reason() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    // Early CUDA 13 builds name `-cu13` packages that NVIDIA published only
    // as placeholders; it ships CUDA 13 libraries under unsuffixed names.
    publish_onnxruntime_gpu(
        &server,
        "/pypi/simple",
        platform,
        &["nvidia-cuda-runtime-cu13~=13.0; extra == \"cuda\""],
    );
    let placeholder = |filename: &str| IndexEntry {
        filename: filename.to_owned(),
        bytes: b"placeholder".to_vec(),
        metadata: None,
        yanked: false,
        sha256: true,
    };
    publish(
        &server,
        "/pypi/simple",
        "nvidia-cuda-runtime-cu13",
        vec![
            placeholder("nvidia_cuda_runtime_cu13-0.0.1.tar.gz"),
            placeholder("nvidia_cuda_runtime_cu13-0.0.0a0-py2.py3-none-any.whl"),
        ],
    );
    let refused = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda13",
        &release(),
        13,
        platform,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        refused.contains("No release of nvidia-cuda-runtime-cu13 on http://127.0.0.1:"),
        "{refused}"
    );
    assert!(
        refused.contains(
            "satisfies nvidia-cuda-runtime-cu13 ~=13.0 (required by onnxruntime-gpu 1.24.2)"
        ),
        "{refused}"
    );
}

#[tokio::test]
async fn extraction_keeps_only_shared_libraries_and_their_licenses() {
    let server = FixtureServer::start().await;
    let temp = tempfile::TempDir::new().unwrap();
    for platform in [WheelPlatform::LinuxX64, WheelPlatform::WindowsX64] {
        let index = format!("/{}/simple", platform.id());
        publish_onnxruntime_gpu(&server, &index, platform, CUDA12_EXTRAS);
        for package in CUDA12_PACKAGES {
            publish(
                &server,
                &index,
                package.project,
                vec![package_entry(platform, package, 64)],
            );
        }
        let mut sources = sources(&server);
        sources.requirement_indexes = vec![format!("{}{index}/", server.base_url)];
        sources.wheel_index = format!("{}{index}/", server.base_url);
        let selection = onnx_cuda::resolve(&sources, "v1.24.2+cuda12", &release(), 12, platform)
            .await
            .unwrap();
        let wheels: Vec<File> = selection
            .wheels
            .iter()
            .map(|wheel| {
                let path = temp.path().join(&wheel.filename);
                let package = CUDA12_PACKAGES
                    .iter()
                    .find(|package| package.project == wheel.project)
                    .unwrap();
                std::fs::write(&path, package_entry(platform, package, 64).bytes).unwrap();
                File::open(path).unwrap()
            })
            .collect();
        let stage = temp.path().join(platform.id());
        std::fs::create_dir(&stage).unwrap();
        onnx_cuda::extract_libraries(wheels, &selection, &stage, platform).unwrap();

        let library_dir = stage.join("cuda/lib");
        let mut names: Vec<String> = std::fs::read_dir(&library_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        let mut expected: Vec<String> = CUDA12_PACKAGES
            .iter()
            .flat_map(|package| package.libraries.iter())
            .map(|(linux, windows)| match platform {
                WheelPlatform::LinuxX64 => (*linux).to_owned(),
                WheelPlatform::WindowsX64 => (*windows).to_owned(),
            })
            .collect();
        expected.sort();
        assert_eq!(names, expected, "only shared libraries, flattened");
        assert!(stage
            .join("cuda/licenses/nvidia-cudnn-cu12/License.txt")
            .is_file());
        let record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(stage.join("cuda/libraries.json")).unwrap())
                .unwrap();
        assert_eq!(record["cuda_major"], 12);
        assert_eq!(
            record["packages"].as_array().unwrap().len(),
            CUDA12_PACKAGES.len()
        );
        let cudnn = record["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["project"] == "nvidia-cudnn-cu12")
            .unwrap();
        assert_eq!(cudnn["libraries"].as_array().unwrap().len(), 2);
        for library in cudnn["libraries"].as_array().unwrap() {
            let name = library["name"].as_str().unwrap();
            assert_eq!(
                library["sha256"],
                sha256_hex(&std::fs::read(library_dir.join(name)).unwrap())
            );
        }
    }
}

#[test]
fn a_wheel_that_ships_a_library_as_a_link_is_refused() {
    use zip::{write::SimpleFileOptions, ZipWriter};
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp
        .path()
        .join("nvidia_link-1.0-py3-none-manylinux_2_27_x86_64.whl");
    let mut writer = ZipWriter::new(File::create(&path).unwrap());
    writer
        .add_symlink(
            "nvidia/link/lib/libcudart.so.12",
            "/usr/lib/libcudart.so.12",
            SimpleFileOptions::default(),
        )
        .unwrap();
    writer.finish().unwrap();
    let selection = onnx_cuda::CudaWheelSelection {
        schema_version: 1,
        tag: "v1.24.2+cuda12".into(),
        platform: WheelPlatform::LinuxX64.id().into(),
        cuda_major: 12,
        requirements: onnx_cuda::RequirementsSource {
            index: "fixture".into(),
            project: "onnxruntime-gpu".into(),
            version: RELEASE.into(),
            wheel: "fixture.whl".into(),
            requires: vec![],
        },
        wheels: vec![onnx_cuda::SelectedWheel {
            project: "nvidia-link".into(),
            version: "1.0".into(),
            filename: "nvidia_link-1.0-py3-none-manylinux_2_27_x86_64.whl".into(),
            url: "http://127.0.0.1/unused".into(),
            sha256: "a".repeat(64),
            size: None,
        }],
    };
    let stage = temp.path().join("stage");
    std::fs::create_dir(&stage).unwrap();
    let refused = onnx_cuda::extract_libraries(
        vec![File::open(&path).unwrap()],
        &selection,
        &stage,
        WheelPlatform::LinuxX64,
    )
    .unwrap_err()
    .to_string();
    assert!(refused.contains("as a link"), "{refused}");
}

#[tokio::test]
async fn a_kept_selection_is_replaced_only_when_a_wheel_is_not_found() {
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::LinuxX64;
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA12_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);
    let selection = onnx_cuda::resolve(
        &sources(&server),
        "v1.24.2+cuda12",
        &release(),
        12,
        platform,
    )
    .await
    .unwrap();
    let check = |server: &FixtureServer| {
        let sources = sources(server);
        let selection = selection.clone();
        async move { onnx_cuda::vanished_wheel(&sources, &selection).await }
    };
    assert_eq!(check(&server).await, None);

    let cufft = CUDA12_PACKAGES
        .iter()
        .find(|package| package.project == "nvidia-cufft-cu12")
        .unwrap();
    let path = format!(
        "/files/pypi/simple/{}",
        wheel_filename(platform, cufft.distribution, cufft.version)
    );
    // A server error decides nothing: the selection is kept.
    server.fail(&path);
    assert_eq!(check(&server).await, None);
    server.state.lock().unwrap().failing.clear();

    // The same file name with another digest is kept too; the verified
    // download refuses it.
    let mut changed = package_entry(platform, cufft, 64);
    changed.bytes = b"rebuilt".to_vec();
    publish(&server, "/pypi/simple", cufft.project, vec![changed]);
    assert_eq!(check(&server).await, None);

    // The file answers 404.
    server.unroute(&path);
    let reason = check(&server)
        .await
        .expect("a missing file replaces the selection")
        .reason;
    assert!(reason.contains("404"), "{reason}");
    assert!(reason.contains("nvidia_cufft_cu12-11.4.1.4"), "{reason}");

    // The index no longer lists it.
    publish(&server, "/pypi/simple", cufft.project, vec![]);
    let reason = check(&server).await.unwrap().reason;
    assert!(
        reason.contains("no longer lists nvidia_cufft_cu12-11.4.1.4"),
        "{reason}"
    );

    // The project itself is gone.
    server.unroute("/pypi/simple/nvidia-cufft-cu12/");
    let reason = check(&server).await.unwrap().reason;
    assert!(
        reason.contains("nvidia-cufft-cu12 is no longer on"),
        "{reason}"
    );
}

fn listed(tag: &str) -> pumas_library::models::GitHubRelease {
    pumas_library::models::GitHubRelease {
        tag_name: tag.into(),
        name: tag.into(),
        published_at: "2026-10-01T00:00:00Z".into(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets: vec![],
        html_url: String::new(),
        total_size: None,
        archive_size: None,
        dependencies_size: None,
        unavailable_reason: None,
        install_availability: None,
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "windows"),
    target_arch = "x86_64"
))]
#[tokio::test]
async fn listed_cuda_builds_carry_their_install_state() {
    use pumas_library::models::InstallAvailability as State;
    let server = FixtureServer::start().await;
    let platform = WheelPlatform::host().unwrap();
    publish_onnxruntime_gpu(&server, "/pypi/simple", platform, CUDA12_EXTRAS);
    publish_cuda12_closure(&server, platform, 64);
    // The CUDA 13 lookup is slow; the CUDA 12 feed cannot be reached.
    let feed13 = "/feed13/simple/onnxruntime-gpu/";
    server.stall(feed13);
    server.fail("/feed12/simple/onnxruntime-gpu/");
    let availability = Arc::new(
        CudaAvailability::new(sources(&server))
            .with_listing_deadline(std::time::Duration::from_millis(300)),
    );
    let tags = [
        "v1.24.2+cpu",
        "v1.24.2+cuda12",
        "v1.24.2+cuda13",
        // Not on PyPI, and its feed answers 503.
        "v1.25.0+cuda12",
    ];
    let list = || async {
        let mut releases: Vec<_> = tags.iter().map(|tag| listed(tag)).collect();
        availability.annotate(&mut releases).await;
        releases
            .into_iter()
            .map(|release| {
                (
                    release.tag_name,
                    (release.install_availability, release.unavailable_reason),
                )
            })
            .collect::<HashMap<_, _>>()
    };

    let first = list().await;
    assert_eq!(first["v1.24.2+cpu"], (None, None));
    assert_eq!(first["v1.24.2+cuda12"], (Some(State::Available), None));
    assert_eq!(first["v1.24.2+cuda13"], (Some(State::Checking), None));
    // A network failure is not a verdict: installing still decides.
    assert_eq!(first["v1.25.0+cuda12"], (Some(State::Unknown), None));
    assert_eq!(server.requests(feed13).len(), 1);

    // The next listing waits on the same lookup instead of starting another.
    let second = list().await;
    assert_eq!(second["v1.24.2+cuda13"].0, Some(State::Checking));
    assert_eq!(server.requests(feed13).len(), 1);

    server.release(feed13);
    let third = list().await;
    let (state, reason) = &third["v1.24.2+cuda13"];
    assert_eq!(*state, Some(State::Unavailable));
    let reason = reason.as_deref().unwrap();
    assert!(
        reason.contains("no published CUDA 13 package metadata"),
        "{reason}"
    );
    assert!(reason.contains("v1.24.2+cuda12 installs"), "{reason}");
    assert_eq!(server.requests(feed13).len(), 1);
    assert_eq!(third["v1.25.0+cuda12"].0, Some(State::Unknown));

    // Settled answers are kept: listing again asks no index.
    let asked = server.requested_paths().len();
    let fourth = list().await;
    assert_eq!(server.requested_paths().len(), asked);
    assert_eq!(fourth, third);

    // A refresh asks again; the unreachable feed now answers.
    server.state.lock().unwrap().failing.clear();
    availability.refresh();
    let fifth = list().await;
    let (state, reason) = &fifth["v1.25.0+cuda12"];
    assert_eq!(*state, Some(State::Unavailable));
    assert!(reason
        .as_deref()
        .unwrap()
        .contains("has no onnxruntime-gpu"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod installs {
    //! Installing ONNX Runtime CUDA builds through the version manager.

    use super::*;
    use crate::version_manager::VersionManager;
    use pumas_library::network::{GitHubAsset, GitHubRelease, ReleasesCache};
    use pumas_library::onnx_runtime_distribution as distribution;
    use std::time::Duration;

    const RELEASE_TAG: &str = "v1.24.2";
    const CUDA_TAG: &str = "v1.24.2+cuda12";
    /// Large enough that a transfer can stop partway and resume.
    const WHEEL_PAYLOAD: usize = 256 * 1024;

    fn cuda_archive_name() -> String {
        distribution::variant_archive_names(
            &release(),
            distribution::OnnxRuntimeVariant::Cuda12,
            "linux",
            "x86_64",
        )
        .remove(0)
    }

    fn cpu_archive_name() -> String {
        distribution::platform_archive_names(&release(), "linux", "x86_64").remove(0)
    }

    /// An upstream archive's layout, with fixture bytes.
    fn runtime_archive(name: &str) -> Vec<u8> {
        let wrapper = name.trim_end_matches(".tgz").to_owned();
        let compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(compressed);
        let mut append = |path: String, bytes: &[u8]| {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, path, bytes).unwrap();
        };
        append(
            format!("{wrapper}/lib/libonnxruntime.so.{RELEASE}"),
            b"onnx runtime library fixture",
        );
        append(
            format!("{wrapper}/lib/libonnxruntime_providers_cuda.so"),
            b"cuda provider fixture",
        );
        append(
            format!("{wrapper}/lib/libonnxruntime_providers_shared.so"),
            b"shared provider fixture",
        );
        let mut link = |name: &str, target: &str| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            archive
                .append_link(&mut header, format!("{wrapper}/lib/{name}"), target)
                .unwrap();
        };
        link(
            "libonnxruntime.so.1",
            &format!("libonnxruntime.so.{RELEASE}"),
        );
        link("libonnxruntime.so", "libonnxruntime.so.1");
        archive.into_inner().unwrap().finish().unwrap()
    }

    /// Publish the release, its CPU and CUDA 12 archives and the CUDA 12
    /// package indexes.
    fn publish_release(server: &FixtureServer, root: &Path) {
        let cuda13 = distribution::variant_archive_names(
            &release(),
            distribution::OnnxRuntimeVariant::Cuda13,
            "linux",
            "x86_64",
        )
        .remove(0);
        let archives: Vec<(u64, String, Vec<u8>)> =
            [cpu_archive_name(), cuda_archive_name(), cuda13]
                .into_iter()
                .enumerate()
                .map(|(index, name)| (2412 + index as u64, runtime_archive(&name), name))
                .map(|(id, bytes, name)| (id, name, bytes))
                .collect();
        let url = |name: &str| format!("{}/archives/{name}", server.base_url);
        server.route(
            &format!("/repos/microsoft/onnxruntime/releases/tags/{RELEASE_TAG}"),
            serde_json::to_vec(&serde_json::json!({
                "tag_name": RELEASE_TAG,
                "assets": archives.iter().map(|(id, name, bytes)| serde_json::json!({
                    "id": id, "name": name, "size": bytes.len(),
                    "browser_download_url": url(name),
                    "digest": format!("sha256:{}", sha256_hex(bytes)),
                })).collect::<Vec<_>>(),
            }))
            .unwrap(),
            "application/json",
        );
        ReleasesCache::new(root.join("launcher-data/cache"), Duration::from_secs(3600))
            .set_disk(
                AppId::OnnxRuntime.github_repo(),
                &[GitHubRelease {
                    tag_name: RELEASE_TAG.into(),
                    name: RELEASE_TAG.into(),
                    published_at: "2026-10-01T00:00:00Z".into(),
                    body: None,
                    tarball_url: None,
                    zipball_url: None,
                    prerelease: false,
                    assets: archives
                        .iter()
                        .map(|(_, name, bytes)| GitHubAsset {
                            name: name.clone(),
                            size: bytes.len() as u64,
                            download_url: url(name),
                            content_type: Some("application/gzip".into()),
                        })
                        .collect(),
                    html_url: format!(
                        "https://github.com/microsoft/onnxruntime/releases/tag/{RELEASE_TAG}"
                    ),
                    total_size: None,
                    archive_size: None,
                    dependencies_size: None,
                    unavailable_reason: None,
                    install_availability: None,
                }],
            )
            .unwrap();
        for (_, name, bytes) in archives {
            server.route(&format!("/archives/{name}"), bytes, "application/gzip");
        }
        let platform = WheelPlatform::LinuxX64;
        publish_onnxruntime_gpu(server, "/pypi/simple", platform, CUDA12_EXTRAS);
        publish_cuda12_closure(server, platform, WHEEL_PAYLOAD);
    }

    async fn test_api(root: &Path) -> pumas_library::PumasApi {
        pumas_library::PumasApi::builder(root)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap()
    }

    async fn onnx_manager(
        root: &Path,
        api: &pumas_library::PumasApi,
        server: &FixtureServer,
    ) -> VersionManager {
        let mut manager = VersionManager::new_with_acquisition(
            root,
            AppId::OnnxRuntime,
            api.acquisition().clone(),
        )
        .await
        .unwrap();
        manager.github_client = Arc::new(
            GitHubClient::with_loopback_api(
                manager.cache_dir(),
                Duration::from_secs(3600),
                server.base_url.clone(),
            )
            .unwrap(),
        );
        manager.cuda_availability = Arc::new(CudaAvailability::new(sources(server)));
        manager
    }

    /// The outcome of one install: `Ok` on success, else the error message.
    async fn install(manager: &VersionManager, tag: &str) -> std::result::Result<(), String> {
        let mut updates = manager
            .install_version(tag)
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                match updates.recv().await {
                    Some(ProgressUpdate::Completed { success: true }) => return Ok(()),
                    Some(ProgressUpdate::Error { message }) => return Err(message),
                    Some(_) => {}
                    None => return Err("installation ended without an outcome".into()),
                }
            }
        })
        .await
        .unwrap()
    }

    fn wheel_path(package: &NvidiaPackage) -> String {
        format!(
            "/files/pypi/simple/{}",
            wheel_filename(
                WheelPlatform::LinuxX64,
                package.distribution,
                package.version
            )
        )
    }

    fn package(project: &str) -> &'static NvidiaPackage {
        CUDA12_PACKAGES
            .iter()
            .find(|package| package.project == project)
            .unwrap()
    }

    #[tokio::test]
    async fn a_cuda_build_installs_its_cuda_libraries_and_resumes_an_interrupted_wheel() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let cudnn = wheel_path(package("nvidia-cudnn-cu12"));
        server.truncate_once(&cudnn);
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;

        let index_reads = || {
            server
                .requested_paths()
                .iter()
                .filter(|path| {
                    path.starts_with("/pypi/")
                        || path.starts_with("/feed")
                        || path.ends_with(".metadata")
                })
                .count()
        };
        let listed = manager.get_available_releases(false).await.unwrap();
        let reason = |tag: &str| {
            listed
                .iter()
                .find(|release| release.tag_name == tag)
                .unwrap_or_else(|| panic!("{tag} is listed"))
                .unavailable_reason
                .clone()
        };
        // The listing knows which CUDA builds install before any click: this
        // release publishes CUDA 12 metadata only.
        assert_eq!(reason(CUDA_TAG), None);
        assert_eq!(reason("v1.24.2+cpu"), None);
        let unavailable = reason("v1.24.2+cuda13").expect("the CUDA 13 build is unavailable");
        assert!(
            unavailable.contains("has no published CUDA 13 package metadata"),
            "{unavailable}"
        );
        assert!(
            unavailable.contains("Builds that install instead: no listed CUDA 13 build installs; v1.24.2+cuda12 installs."),
            "{unavailable}"
        );

        // The answers are kept: listing again asks no index.
        let after_listing = index_reads();
        assert!(after_listing > 0);
        let again = manager.get_available_releases(false).await.unwrap();
        assert_eq!(index_reads(), after_listing);
        assert!(again
            .iter()
            .any(|release| release.tag_name == "v1.24.2+cuda13"
                && release.unavailable_reason.is_some()));

        // A CPU build installs as before, without reading a package index.
        install(&manager, "v1.24.2+cpu").await.unwrap();
        assert_eq!(index_reads(), after_listing);
        assert!(!manager
            .version_path("v1.24.2+cpu")
            .unwrap()
            .join("cuda")
            .exists());

        install(&manager, CUDA_TAG).await.unwrap();

        // The interrupted wheel continued from the bytes it had.
        let requests = server.requests(&cudnn);
        assert_eq!(requests.len(), 2, "{requests:?}");
        assert_eq!(requests[0], None);
        let resumed_from = requests[1]
            .as_deref()
            .and_then(|range| range.strip_prefix("bytes="))
            .and_then(|range| range.strip_suffix('-'))
            .and_then(|start| start.parse::<usize>().ok())
            .expect("the second request continues with a byte range");
        assert!(resumed_from > 0, "{requests:?}");

        let version_dir = manager.version_path(CUDA_TAG).unwrap();
        assert!(version_dir
            .join(distribution::library_relative_path())
            .is_file());
        assert!(version_dir
            .join("lib/libonnxruntime_providers_cuda.so")
            .is_file());
        let library_dir = version_dir.join(distribution::CUDA_LIBRARY_DIR);
        for package in CUDA12_PACKAGES {
            for (library, _) in package.libraries {
                let installed = library_dir.join(library);
                assert!(installed.is_file(), "{}", installed.display());
                assert_eq!(
                    std::fs::metadata(&installed).unwrap().len(),
                    WHEEL_PAYLOAD as u64
                );
            }
        }
        assert!(!library_dir.join("static_part.a").exists());
        assert_eq!(
            distribution::installed_cuda_library_dir(&version_dir).as_deref(),
            Some(library_dir.as_path())
        );
        // The attempt's selection left with its workspace.
        let leftovers: Vec<_> = std::fs::read_dir(manager.versions_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with(".llama-install-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        // Installing the unavailable build anyway is refused with the same
        // reason and the builds that install instead.
        let refused = install(&manager, "v1.24.2+cuda13").await.unwrap_err();
        assert!(
            refused.contains("has no published CUDA 13 package metadata"),
            "{refused}"
        );
        assert!(
            refused.contains("Builds that install instead:")
                && refused.contains("v1.24.2+cuda12 installs"),
            "{refused}"
        );
        assert!(manager
            .version_path("v1.24.2+cuda13")
            .is_ok_and(|path| !path.exists()));

        // Removing the version removes its CUDA libraries with it.
        manager.remove_version(CUDA_TAG).await.unwrap();
        assert!(!version_dir.exists());
        manager.shutdown_installations().await.unwrap();
    }

    #[tokio::test]
    async fn cancelling_stops_a_cuda_install_waiting_on_a_package_index() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let page = "/pypi/simple/onnxruntime-gpu/";
        server.stall(page);
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;

        let mut updates = manager.install_version(CUDA_TAG).await.unwrap();
        tokio::time::timeout(Duration::from_secs(20), async {
            while server.requests(page).is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert!(manager.cancel_installation().await.unwrap());
        let outcome = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match updates.recv().await {
                    Some(ProgressUpdate::Error { message }) => return message,
                    Some(ProgressUpdate::Completed { success: true }) => {
                        panic!("a cancelled install must not complete")
                    }
                    Some(_) => {}
                    None => return String::new(),
                }
            }
        })
        .await
        .expect("cancel does not wait for the stalled index");
        assert!(outcome.contains("cancelled"), "{outcome}");
        assert!(manager
            .version_path(CUDA_TAG)
            .is_ok_and(|path| !path.exists()));
        tokio::time::timeout(Duration::from_secs(10), manager.shutdown_installations())
            .await
            .expect("shutdown does not wait for the stalled index")
            .ok();
    }

    #[tokio::test]
    async fn a_retried_cuda_install_downloads_the_selection_it_started_with() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let nvjitlink = wheel_path(package("nvidia-nvjitlink-cu12"));
        let nvjitlink_bytes = package_entry(
            WheelPlatform::LinuxX64,
            package("nvidia-nvjitlink-cu12"),
            WHEEL_PAYLOAD,
        )
        .bytes;
        server.unroute(&nvjitlink);
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;

        let failed = install(&manager, CUDA_TAG).await.unwrap_err();
        assert!(!failed.is_empty());
        assert!(manager
            .version_path(CUDA_TAG)
            .is_ok_and(|path| !path.exists()));
        // Resolving reads core metadata; checking a kept selection does not.
        let metadata_reads = || {
            server
                .requested_paths()
                .iter()
                .filter(|path| path.ends_with(".metadata"))
                .count()
        };
        let index_reads = metadata_reads();
        assert!(index_reads > 0);

        // NVIDIA publishes a newer cuDNN before the retry, and the missing
        // wheel becomes available.
        let cudnn = package("nvidia-cudnn-cu12");
        let newer = IndexEntry::wheel(
            &wheel_filename(WheelPlatform::LinuxX64, cudnn.distribution, "9.29.0.1"),
            nvidia_wheel(
                WheelPlatform::LinuxX64,
                cudnn.distribution,
                "9.29.0.1",
                cudnn.component,
                cudnn.libraries,
                64,
            ),
            cudnn.requires,
        );
        publish(
            &server,
            "/pypi/simple",
            cudnn.project,
            vec![
                package_entry(WheelPlatform::LinuxX64, cudnn, WHEEL_PAYLOAD),
                newer,
            ],
        );
        server.route(&nvjitlink, nvjitlink_bytes, "application/octet-stream");
        let cudnn_requests = server.requests(&wheel_path(cudnn)).len();

        install(&manager, CUDA_TAG).await.unwrap();

        let paths = server.requested_paths();
        assert!(
            !paths.iter().any(|path| path.contains("9.29.0.1")),
            "the retry keeps the selection it started with: {paths:?}"
        );
        assert_eq!(
            metadata_reads(),
            index_reads,
            "the retry does not resolve again"
        );
        assert_eq!(
            server.requests(&wheel_path(cudnn)).len(),
            cudnn_requests,
            "a wheel the failed attempt completed is not downloaded again"
        );
        let version_dir = manager.version_path(CUDA_TAG).unwrap();
        assert!(version_dir.join("cuda/lib/libcublasLt.so.12").is_file());
        manager.shutdown_installations().await.ok();
    }

    /// The `.whl` downloads of `server` for `filename`.
    fn downloads(server: &FixtureServer, filename: &str) -> usize {
        server
            .state
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|(path, _)| {
                !path.starts_with("HEAD ") && path.ends_with(&format!("/{filename}"))
            })
            .count()
    }

    fn install_log(root: &Path) -> String {
        let logs = root.join("launcher-data/logs");
        std::fs::read_dir(&logs)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with("install-onnx-runtime-")
                    })
                    .map(|entry| std::fs::read_to_string(entry.path()).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    async fn retained_record_phases(api: &pumas_library::PumasApi) -> Vec<String> {
        api.acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .filter(|record| record.demand.consumer == "runtime.onnx-runtime")
            .map(|record| format!("{:?}", record.phase))
            .collect()
    }

    #[tokio::test]
    async fn a_retry_whose_saved_wheel_vanished_resolves_again_and_keeps_finished_downloads() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let platform = WheelPlatform::LinuxX64;
        let nvjitlink = package("nvidia-nvjitlink-cu12");
        let old_wheel = wheel_filename(platform, nvjitlink.distribution, nvjitlink.version);
        // The last wheel of the set cannot be fetched, so the attempt fails
        // after the archive and the other wheels finished.
        server.unroute(&wheel_path(nvjitlink));
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;
        install(&manager, CUDA_TAG).await.unwrap_err();
        let cudnn = package("nvidia-cudnn-cu12");
        let cudnn_wheel = wheel_filename(platform, cudnn.distribution, cudnn.version);
        assert_eq!(downloads(&server, &cudnn_wheel), 1);
        let archive_downloads = server
            .requested_paths()
            .iter()
            .filter(|path| path.starts_with("/archives/") && path.contains("gpu_cuda12"))
            .count();
        assert_eq!(archive_downloads, 1);

        // NVIDIA replaces that release: the index no longer lists it and its
        // file answers 404.
        let newer = IndexEntry::wheel(
            &wheel_filename(platform, nvjitlink.distribution, "12.9.90"),
            nvidia_wheel(
                platform,
                nvjitlink.distribution,
                "12.9.90",
                nvjitlink.component,
                nvjitlink.libraries,
                64,
            ),
            nvjitlink.requires,
        );
        publish(&server, "/pypi/simple", nvjitlink.project, vec![newer]);

        install(&manager, CUDA_TAG).await.unwrap();

        // The selection was resolved again with the replacement.
        let version_dir = manager.version_path(CUDA_TAG).unwrap();
        let record: serde_json::Value = serde_json::from_slice(
            &std::fs::read(version_dir.join("cuda/libraries.json")).unwrap(),
        )
        .unwrap();
        let versions: Vec<_> = record["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|package| {
                format!(
                    "{} {}",
                    package["project"].as_str().unwrap(),
                    package["version"].as_str().unwrap()
                )
            })
            .collect();
        assert!(
            versions.contains(&"nvidia-nvjitlink-cu12 12.9.90".to_owned()),
            "{versions:?}"
        );
        assert!(version_dir.join("cuda/lib/libnvJitLink.so.12").is_file());
        // Finished downloads still selected were kept, not fetched again.
        assert_eq!(downloads(&server, &cudnn_wheel), 1);
        assert_eq!(
            server
                .requested_paths()
                .iter()
                .filter(|path| path.starts_with("/archives/") && path.contains("gpu_cuda12"))
                .count(),
            archive_downloads
        );
        // The old download was withdrawn, not left to be resumed.
        let phases = retained_record_phases(&api).await;
        assert!(
            phases.iter().any(|phase| phase == "Withdrawn"),
            "{phases:?}"
        );
        assert!(
            !phases
                .iter()
                .any(|phase| phase == "Transferring" || phase == "FilesReady"),
            "{phases:?}"
        );
        // The install log says why.
        let log = install_log(root.path());
        assert!(
            log.contains("Replaced the earlier install attempt"),
            "{log}"
        );
        assert!(log.contains(&old_wheel), "{log}");
        let leftovers: Vec<_> = std::fs::read_dir(manager.versions_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| {
                name.starts_with(".llama-install-") || name.starts_with(".native-carry-")
            })
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        manager.shutdown_installations().await.ok();
    }

    #[tokio::test]
    async fn a_retry_whose_saved_wheel_answers_a_server_error_keeps_its_selection() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let platform = WheelPlatform::LinuxX64;
        let nvjitlink = package("nvidia-nvjitlink-cu12");
        let wheel = wheel_path(nvjitlink);
        server.fail(&wheel);
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;
        install(&manager, CUDA_TAG).await.unwrap_err();

        // A newer release appears, but the saved file only answers 503: the
        // retry keeps its selection instead of resolving again.
        let newer = IndexEntry::wheel(
            &wheel_filename(platform, nvjitlink.distribution, "12.9.90"),
            b"newer".to_vec(),
            nvjitlink.requires,
        );
        publish(
            &server,
            "/pypi/simple",
            nvjitlink.project,
            vec![package_entry(platform, nvjitlink, WHEEL_PAYLOAD), newer],
        );
        install(&manager, CUDA_TAG).await.unwrap_err();
        let phases = retained_record_phases(&api).await;
        assert!(
            !phases.iter().any(|phase| phase == "Withdrawn"),
            "{phases:?}"
        );
        assert!(!install_log(root.path()).contains("Replaced the earlier install attempt"));

        // Once the file answers, the same selection finishes.
        server.state.lock().unwrap().failing.clear();
        install(&manager, CUDA_TAG).await.unwrap();
        assert!(!server
            .requested_paths()
            .iter()
            .any(|path| path.ends_with("12.9.90-py3-none-manylinux_2_27_x86_64.whl")));
        let record = std::fs::read_to_string(
            manager
                .version_path(CUDA_TAG)
                .unwrap()
                .join("cuda/libraries.json"),
        )
        .unwrap();
        assert!(record.contains("12.9.86"), "{record}");
        manager.shutdown_installations().await.ok();
    }

    /// The workspace of the live install attempt of `tag`.
    fn live_workspace(manager: &VersionManager) -> PathBuf {
        std::fs::read_dir(manager.versions_dir())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(".llama-install-") && !name.ends_with(".lock")
                    })
                    && path.is_dir()
            })
            .expect("a live install attempt")
    }

    #[tokio::test]
    async fn a_retry_checks_only_the_wheels_it_still_needs() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let nvjitlink = package("nvidia-nvjitlink-cu12");
        let missing = wheel_path(nvjitlink);
        let missing_bytes = package_entry(WheelPlatform::LinuxX64, nvjitlink, WHEEL_PAYLOAD).bytes;
        server.unroute(&missing);
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;
        install(&manager, CUDA_TAG).await.unwrap_err();

        // Every finished wheel now answers 404; the retry does not ask, so
        // it neither replaces the set nor fails on them.
        for package in CUDA12_PACKAGES
            .iter()
            .filter(|package| package.project != nvjitlink.project)
        {
            server.unroute(&wheel_path(package));
        }
        server.route(&missing, missing_bytes, "application/octet-stream");
        install(&manager, CUDA_TAG).await.unwrap();
        let heads: Vec<_> = server
            .requested_paths()
            .into_iter()
            .filter(|path| path.starts_with("HEAD "))
            .collect();
        assert_eq!(heads, vec![format!("HEAD {missing}")]);
        assert!(!install_log(root.path()).contains("Replaced the earlier install attempt"));
        manager.shutdown_installations().await.ok();
    }

    #[tokio::test]
    async fn a_retry_with_an_unusable_kept_selection_replaces_a_differing_download() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let platform = WheelPlatform::LinuxX64;
        let nvjitlink = package("nvidia-nvjitlink-cu12");
        let missing = wheel_path(nvjitlink);
        let missing_bytes = package_entry(platform, nvjitlink, WHEEL_PAYLOAD).bytes;
        server.unroute(&missing);
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;
        install(&manager, CUDA_TAG).await.unwrap_err();

        // The kept selection cannot be read any more, and the index now
        // resolves to a different set than the retained download holds.
        std::fs::write(
            live_workspace(&manager).join("cuda-wheels.json"),
            b"{ not json",
        )
        .unwrap();
        let cudnn = package("nvidia-cudnn-cu12");
        let newer = IndexEntry::wheel(
            &wheel_filename(platform, cudnn.distribution, "9.29.0.1"),
            nvidia_wheel(
                platform,
                cudnn.distribution,
                "9.29.0.1",
                cudnn.component,
                cudnn.libraries,
                64,
            ),
            cudnn.requires,
        );
        publish(
            &server,
            "/pypi/simple",
            cudnn.project,
            vec![package_entry(platform, cudnn, WHEEL_PAYLOAD), newer],
        );
        server.route(&missing, missing_bytes, "application/octet-stream");

        install(&manager, CUDA_TAG).await.unwrap();
        let record = std::fs::read_to_string(
            manager
                .version_path(CUDA_TAG)
                .unwrap()
                .join("cuda/libraries.json"),
        )
        .unwrap();
        assert!(record.contains("9.29.0.1"), "{record}");
        let log = install_log(root.path());
        assert!(
            log.contains("Replaced the earlier install attempt"),
            "{log}"
        );
        assert!(log.contains("differs from the current one"), "{log}");
        let phases = retained_record_phases(&api).await;
        assert!(
            phases.iter().any(|phase| phase == "Withdrawn"),
            "{phases:?}"
        );
        manager.shutdown_installations().await.ok();
    }

    #[tokio::test]
    async fn a_wheel_whose_file_is_gone_but_still_listed_is_not_selected_again() {
        let root = tempfile::TempDir::new().unwrap();
        let server = FixtureServer::start().await;
        publish_release(&server, root.path());
        let nvjitlink = package("nvidia-nvjitlink-cu12");
        server.unroute(&wheel_path(nvjitlink));
        let api = test_api(root.path()).await;
        let manager = onnx_manager(root.path(), &api, &server).await;
        install(&manager, CUDA_TAG).await.unwrap_err();

        // The index still lists the file, which answers 404, and no other
        // release has a host wheel: resolving without it is refused with the
        // reason every time, instead of selecting the same file again, and
        // the earlier attempt is left as it was.
        for _ in 0..2 {
            let refused = install(&manager, CUDA_TAG).await.unwrap_err();
            // nvJitLink has no other release, so cuFFT, which requires it,
            // has none that can install either.
            assert!(
                refused.contains("No release of nvidia-cufft-cu12")
                    && refused.contains("cannot be installed"),
                "{refused}"
            );
        }
        let phases = retained_record_phases(&api).await;
        assert!(
            !phases.iter().any(|phase| phase == "Withdrawn"),
            "{phases:?}"
        );
        assert!(!install_log(root.path()).contains("Replaced the earlier install attempt"));
        manager.shutdown_installations().await.ok();
    }
}
