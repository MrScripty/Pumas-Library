//! The parts of Python packaging that Pumas reads without a Python.
//!
//! Pumas resolves NVIDIA's CUDA library wheels from package index metadata and
//! extracts their shared libraries itself, so it needs: version ordering and
//! specifiers (PEP 440), `Requires-Dist` requirements and their environment
//! markers (PEP 508), wheel file names (PEP 427), the JSON simple index API
//! (PEP 691) and core metadata served beside a wheel (PEP 658/714).
//!
//! Only what that resolution uses is implemented. Anything outside it is
//! refused with a reason rather than approximated.

use pumas_library::{PumasError, Result};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

/// The largest index page Pumas reads. NVIDIA and ONNX Runtime pages list a
/// few hundred files and stay well below a megabyte.
const MAX_INDEX_PAGE_BYTES: usize = 32 * 1024 * 1024;
/// The largest core metadata file Pumas reads.
const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
const SIMPLE_JSON: &str = "application/vnd.pypi.simple.v1+json";

fn refused(message: impl Into<String>) -> PumasError {
    PumasError::InstallationFailed {
        message: message.into(),
    }
}

/// A project name in its normalized form (PEP 503).
pub(super) fn normalize_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut separator = false;
    for character in name.chars() {
        if matches!(character, '-' | '_' | '.') {
            separator = true;
            continue;
        }
        if separator && !normalized.is_empty() {
            normalized.push('-');
        }
        separator = false;
        normalized.push(character.to_ascii_lowercase());
    }
    normalized
}

/// A PEP 440 version. Local version labels are ignored.
#[derive(Clone, Debug)]
pub(super) struct PyVersion {
    epoch: u64,
    release: Vec<u64>,
    pre: Option<(u8, u64)>,
    post: Option<u64>,
    dev: Option<u64>,
    text: String,
}

fn version_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"^v?(?:(\d+)!)?(\d+(?:\.\d+)*)(?:[-_.]?(a|b|c|rc|alpha|beta|pre|preview)[-_.]?(\d+)?)?(?:-(\d+)|[-_.]?(post|rev|r)[-_.]?(\d+)?)?(?:[-_.]?(dev)[-_.]?(\d+)?)?$",
        )
        .expect("PEP 440 pattern")
    })
}

impl PyVersion {
    pub(super) fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        let public = trimmed
            .split_once('+')
            .map_or(trimmed, |(public, _)| public);
        let lowered = public.to_ascii_lowercase();
        let captures = version_pattern().captures(&lowered)?;
        let number = |index: usize| -> Option<Option<u64>> {
            match captures.get(index) {
                None => Some(None),
                Some(value) => value.as_str().parse().ok().map(Some),
            }
        };
        let epoch = number(1)?.unwrap_or(0);
        let release = captures
            .get(2)?
            .as_str()
            .split('.')
            .map(str::parse)
            .collect::<std::result::Result<Vec<u64>, _>>()
            .ok()?;
        let pre = match captures.get(3) {
            None => None,
            Some(kind) => {
                let rank = match kind.as_str() {
                    "a" | "alpha" => 0,
                    "b" | "beta" => 1,
                    _ => 2,
                };
                Some((rank, number(4)?.unwrap_or(0)))
            }
        };
        let post = match (captures.get(5), captures.get(6)) {
            (Some(_), _) => number(5)?,
            (None, Some(_)) => Some(number(7)?.unwrap_or(0)),
            (None, None) => None,
        };
        let dev = match captures.get(8) {
            Some(_) => Some(number(9)?.unwrap_or(0)),
            None => None,
        };
        Some(Self {
            epoch,
            release,
            pre,
            post,
            dev,
            text: trimmed.to_owned(),
        })
    }

    /// A final or post release: not a pre-release or a development release.
    pub(super) fn is_final(&self) -> bool {
        self.pre.is_none() && self.dev.is_none()
    }

    pub(super) fn major(&self) -> u64 {
        self.release.first().copied().unwrap_or(0)
    }

    pub(super) fn as_str(&self) -> &str {
        &self.text
    }

    fn release_cmp(&self, other: &Self) -> Ordering {
        let length = self.release.len().max(other.release.len());
        (0..length)
            .map(|index| {
                let left = self.release.get(index).copied().unwrap_or(0);
                let right = other.release.get(index).copied().unwrap_or(0);
                left.cmp(&right)
            })
            .find(|ordering| ordering.is_ne())
            .unwrap_or(Ordering::Equal)
    }

    /// Whether the first `prefix.len()` release segments equal `prefix`.
    fn release_starts_with(&self, prefix: &[u64]) -> bool {
        prefix
            .iter()
            .enumerate()
            .all(|(index, value)| self.release.get(index).copied().unwrap_or(0) == *value)
    }

    fn sort_key(&self) -> (i8, u8, u64, i8, u64, i8, u64) {
        // A development release of a final sorts before its pre-releases.
        let pre = match (self.pre, self.post, self.dev) {
            (None, None, Some(_)) => (-1, 0, 0),
            (Some((kind, number)), _, _) => (0, kind, number),
            _ => (1, 0, 0),
        };
        let post = self.post.map_or((-1, 0), |number| (0, number));
        let dev = self.dev.map_or((1, 0), |number| (0, number));
        (pre.0, pre.1, pre.2, post.0, post.1, dev.0, dev.1)
    }
}

impl PartialEq for PyVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for PyVersion {}

impl PartialOrd for PyVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PyVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.epoch
            .cmp(&other.epoch)
            .then_with(|| self.release_cmp(other))
            .then_with(|| self.sort_key().cmp(&other.sort_key()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operator {
    Compatible,
    Equal,
    NotEqual,
    LessEqual,
    GreaterEqual,
    Less,
    Greater,
    Arbitrary,
}

/// One PEP 440 version specifier clause, such as `~=12.0` or `>=9,<10`'s parts.
#[derive(Clone, Debug)]
pub(super) struct Specifier {
    operator: Operator,
    version: Option<PyVersion>,
    /// `==1.2.*` and `!=1.2.*` compare this release prefix.
    prefix: Option<Vec<u64>>,
    text: String,
}

impl Specifier {
    pub(super) fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        let (operator, rest) = [
            ("===", Operator::Arbitrary),
            ("~=", Operator::Compatible),
            ("==", Operator::Equal),
            ("!=", Operator::NotEqual),
            ("<=", Operator::LessEqual),
            (">=", Operator::GreaterEqual),
            ("<", Operator::Less),
            (">", Operator::Greater),
        ]
        .into_iter()
        .find_map(|(symbol, operator)| {
            trimmed
                .strip_prefix(symbol)
                .map(|rest| (operator, rest.trim()))
        })?;
        if rest.is_empty() {
            return None;
        }
        if operator == Operator::Arbitrary {
            return Some(Self {
                operator,
                version: None,
                prefix: None,
                text: trimmed.to_owned(),
            });
        }
        if let Some(prefix) = rest.strip_suffix(".*") {
            if !matches!(operator, Operator::Equal | Operator::NotEqual) {
                return None;
            }
            let prefix = PyVersion::parse(prefix)?;
            return Some(Self {
                operator,
                prefix: Some(prefix.release.clone()),
                version: Some(prefix),
                text: trimmed.to_owned(),
            });
        }
        let version = PyVersion::parse(rest)?;
        if operator == Operator::Compatible && version.release.len() < 2 {
            return None;
        }
        Some(Self {
            operator,
            version: Some(version),
            prefix: None,
            text: trimmed.to_owned(),
        })
    }

    pub(super) fn contains(&self, candidate: &PyVersion) -> bool {
        let Some(version) = &self.version else {
            // `===` compares the exact string.
            return self
                .text
                .trim_start_matches("===")
                .trim()
                .eq_ignore_ascii_case(candidate.as_str());
        };
        match (self.operator, &self.prefix) {
            (Operator::Equal, Some(prefix)) => {
                candidate.epoch == version.epoch && candidate.release_starts_with(prefix)
            }
            (Operator::NotEqual, Some(prefix)) => {
                !(candidate.epoch == version.epoch && candidate.release_starts_with(prefix))
            }
            (Operator::Compatible, _) => {
                let prefix = &version.release[..version.release.len() - 1];
                candidate >= version
                    && candidate.epoch == version.epoch
                    && candidate.release_starts_with(prefix)
            }
            (Operator::Equal, None) => candidate == version,
            (Operator::NotEqual, None) => candidate != version,
            (Operator::LessEqual, _) => candidate <= version,
            (Operator::GreaterEqual, _) => candidate >= version,
            (Operator::Less, _) => {
                candidate < version
                    && !(version.pre.is_none()
                        && candidate.pre.is_some()
                        && candidate.release_cmp(version).is_eq())
            }
            (Operator::Greater, _) => {
                candidate > version
                    && !(version.post.is_none()
                        && candidate.post.is_some()
                        && candidate.release_cmp(version).is_eq())
            }
            (Operator::Arbitrary, _) => false,
        }
    }

    /// The release a lower-bounding clause starts from: `~=13.0`, `==13.*`,
    /// `>=13` and `==13.0.1` all name release 13.
    pub(super) fn lower_bound(&self) -> Option<&PyVersion> {
        match self.operator {
            Operator::Compatible | Operator::Equal | Operator::GreaterEqual => {
                self.version.as_ref()
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for Specifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

/// The platform a CUDA library wheel must run on, as markers and wheel tags
/// see it. ONNX Runtime publishes CUDA builds for these two only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WheelPlatform {
    LinuxX64,
    WindowsX64,
}

impl WheelPlatform {
    /// The running platform, if ONNX Runtime has CUDA builds for it.
    pub(super) fn host() -> Option<Self> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => Some(Self::LinuxX64),
            ("windows", "x86_64") => Some(Self::WindowsX64),
            _ => None,
        }
    }

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::LinuxX64 => "linux-x86_64",
            Self::WindowsX64 => "windows-x86_64",
        }
    }

    fn marker_value(self, variable: &str) -> Option<&'static str> {
        Some(match (self, variable) {
            (Self::LinuxX64, "platform_system") => "Linux",
            (Self::LinuxX64, "sys_platform") => "linux",
            (Self::LinuxX64, "platform_machine") => "x86_64",
            (Self::LinuxX64, "os_name") => "posix",
            (Self::WindowsX64, "platform_system") => "Windows",
            (Self::WindowsX64, "sys_platform") => "win32",
            (Self::WindowsX64, "platform_machine") => "AMD64",
            (Self::WindowsX64, "os_name") => "nt",
            _ => return None,
        })
    }

    /// How well a wheel platform tag suits this platform: `Some(0)` for a
    /// platform build, `Some(1)` for a pure (`any`) wheel, `None` otherwise.
    fn tag_rank(self, tag: &str) -> Option<u8> {
        if tag == "any" {
            return Some(1);
        }
        let matches = match self {
            // glibc builds only: ONNX Runtime's own Linux build needs glibc.
            Self::LinuxX64 => linux_tag_supported(tag, host_glibc()),
            Self::WindowsX64 => tag == "win_amd64",
        };
        matches.then_some(0)
    }

    /// Whether `name` is a shared library the dynamic loader of this platform
    /// loads by name: `libcudart.so.12` on Linux, `cudart64_12.dll` on Windows.
    pub(super) fn is_shared_library(self, name: &str) -> bool {
        match self {
            Self::LinuxX64 => {
                let Some((stem, versions)) = name.split_once(".so") else {
                    return false;
                };
                name.starts_with("lib")
                    && stem.len() > 3
                    && (versions.is_empty()
                        || versions.strip_prefix('.').is_some_and(|numbers| {
                            numbers.split('.').all(|part| {
                                !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                            })
                        }))
            }
            Self::WindowsX64 => name.len() > 4 && name.to_ascii_lowercase().ends_with(".dll"),
        }
    }
}

/// The glibc version a manylinux x86_64 platform tag requires (PEP 600, with
/// the PEP 513/571/599 aliases), or `None` for any other tag.
fn manylinux_x86_64_glibc(tag: &str) -> Option<(u32, u32)> {
    match tag {
        "manylinux1_x86_64" => Some((2, 5)),
        "manylinux2010_x86_64" => Some((2, 12)),
        "manylinux2014_x86_64" => Some((2, 17)),
        _ => {
            let (major, minor) = tag
                .strip_prefix("manylinux_")?
                .strip_suffix("_x86_64")?
                .split_once('_')?;
            Some((major.parse().ok()?, minor.parse().ok()?))
        }
    }
}

/// Whether a Linux x86_64 wheel tag runs with `glibc`: a manylinux tag whose
/// glibc requirement the host meets. An unknown host glibc admits every
/// manylinux tag (a library resolved for another machine).
fn linux_tag_supported(tag: &str, glibc: Option<(u32, u32)>) -> bool {
    manylinux_x86_64_glibc(tag)
        .is_some_and(|required| glibc.is_none_or(|available| required <= available))
}

/// The glibc version of this process, when it runs on glibc.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[allow(unsafe_code)]
fn host_glibc() -> Option<(u32, u32)> {
    static VERSION: OnceLock<Option<(u32, u32)>> = OnceLock::new();
    *VERSION.get_or_init(|| {
        // SAFETY: gnu_get_libc_version takes no arguments and returns a
        // pointer to a static NUL-terminated string owned by glibc.
        let text = unsafe { std::ffi::CStr::from_ptr(libc::gnu_get_libc_version()) };
        let mut parts = text.to_str().ok()?.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        Some((major, minor))
    })
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn host_glibc() -> Option<(u32, u32)> {
    None
}

/// A parsed `Requires-Dist` requirement (PEP 508), without URL references.
#[derive(Clone, Debug)]
pub(super) struct Requirement {
    pub(super) name: String,
    pub(super) extras: BTreeSet<String>,
    pub(super) specifiers: Vec<Specifier>,
    marker: Option<Marker>,
    pub(super) text: String,
}

fn requirement_name_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^([A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?)\s*").expect("PEP 508 name")
    })
}

impl Requirement {
    pub(super) fn parse(text: &str) -> std::result::Result<Self, String> {
        let invalid = |why: &str| format!("requirement `{text}` {why}");
        let (requirement, marker) = split_marker(text);
        let captures = requirement_name_pattern()
            .captures(requirement)
            .ok_or_else(|| invalid("has no project name"))?;
        let name = normalize_name(&captures[1]);
        let mut rest = requirement[captures.get(0).map_or(0, |m| m.end())..].trim();
        let mut extras = BTreeSet::new();
        if let Some(after) = rest.strip_prefix('[') {
            let (list, tail) = after
                .split_once(']')
                .ok_or_else(|| invalid("has an unterminated extras list"))?;
            extras = list
                .split(',')
                .map(str::trim)
                .filter(|extra| !extra.is_empty())
                .map(normalize_name)
                .collect();
            rest = tail.trim();
        }
        if rest.starts_with('@') {
            return Err(invalid(
                "names a direct URL, which a package index cannot verify",
            ));
        }
        let rest = rest
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
            .unwrap_or(rest)
            .trim();
        let specifiers = if rest.is_empty() {
            Vec::new()
        } else {
            rest.split(',')
                .map(|clause| {
                    Specifier::parse(clause).ok_or_else(|| {
                        invalid(&format!("has an unsupported version clause `{clause}`"))
                    })
                })
                .collect::<std::result::Result<_, _>>()?
        };
        let marker = marker
            .map(|marker| Marker::parse(marker).map_err(|why| invalid(&why)))
            .transpose()?;
        Ok(Self {
            name,
            extras,
            specifiers,
            marker,
            text: text.trim().to_owned(),
        })
    }

    /// Whether this requirement applies on `platform` for a distribution
    /// requested with `extras` (normalized names).
    pub(super) fn applies(&self, platform: WheelPlatform, extras: &BTreeSet<String>) -> bool {
        self.marker.as_ref().is_none_or(|marker| {
            if extras.is_empty() {
                marker.evaluate(platform, None)
            } else {
                extras
                    .iter()
                    .any(|extra| marker.evaluate(platform, Some(extra)))
            }
        })
    }

    /// Whether this requirement applies only because of one of `extras`:
    /// it applies with them and not without them.
    pub(super) fn added_by_extras(
        &self,
        platform: WheelPlatform,
        extras: &BTreeSet<String>,
    ) -> bool {
        self.applies(platform, extras) && !self.applies(platform, &BTreeSet::new())
    }

    pub(super) fn allows(&self, version: &PyVersion) -> bool {
        self.specifiers
            .iter()
            .all(|specifier| specifier.contains(version))
    }

    /// The project and version range, without its marker: `nvidia-cudnn-cu12 ~=9.0`.
    pub(super) fn constraint(&self) -> String {
        if self.specifiers.is_empty() {
            return format!("{} (any version)", self.name);
        }
        let ranges: Vec<String> = self.specifiers.iter().map(ToString::to_string).collect();
        format!("{} {}", self.name, ranges.join(","))
    }
}

/// Split `requirement ; marker` at the first semicolon outside quotes.
fn split_marker(text: &str) -> (&str, Option<&str>) {
    let mut quote = None;
    for (index, character) in text.char_indices() {
        match (quote, character) {
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), _) if open == character => quote = None,
            (None, ';') => return (text[..index].trim(), Some(text[index + 1..].trim())),
            _ => {}
        }
    }
    (text.trim(), None)
}

#[derive(Clone, Debug, PartialEq)]
enum MarkerToken {
    Variable(String),
    Literal(String),
    Operator(String),
    And,
    Or,
    Open,
    Close,
}

/// A PEP 508 environment marker expression.
#[derive(Clone, Debug)]
enum Marker {
    And(Box<Marker>, Box<Marker>),
    Or(Box<Marker>, Box<Marker>),
    Compare {
        left: MarkerValue,
        operator: String,
        right: MarkerValue,
    },
}

#[derive(Clone, Debug)]
enum MarkerValue {
    Variable(String),
    Literal(String),
}

fn tokenize_marker(text: &str) -> std::result::Result<Vec<MarkerToken>, String> {
    let mut tokens = Vec::new();
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            index += 1;
        } else if character == '(' {
            tokens.push(MarkerToken::Open);
            index += 1;
        } else if character == ')' {
            tokens.push(MarkerToken::Close);
            index += 1;
        } else if character == '"' || character == '\'' {
            let end = characters[index + 1..]
                .iter()
                .position(|candidate| *candidate == character)
                .ok_or("has an unterminated marker string")?;
            tokens.push(MarkerToken::Literal(
                characters[index + 1..index + 1 + end].iter().collect(),
            ));
            index += end + 2;
        } else if "=!<>~".contains(character) {
            let mut end = index;
            while end < characters.len() && "=!<>~".contains(characters[end]) {
                end += 1;
            }
            tokens.push(MarkerToken::Operator(
                characters[index..end].iter().collect(),
            ));
            index = end;
        } else if character.is_ascii_alphabetic() || character == '_' {
            let mut end = index;
            while end < characters.len()
                && (characters[end].is_ascii_alphanumeric() || matches!(characters[end], '_' | '.'))
            {
                end += 1;
            }
            let word: String = characters[index..end].iter().collect();
            index = end;
            match word.as_str() {
                "and" => tokens.push(MarkerToken::And),
                "or" => tokens.push(MarkerToken::Or),
                "in" => tokens.push(MarkerToken::Operator("in".into())),
                "not" => {
                    let rest: String = characters[index..].iter().collect();
                    let trimmed = rest.trim_start();
                    let Some(after) = trimmed.strip_prefix("in") else {
                        return Err("uses `not` without `in`".into());
                    };
                    index = characters.len() - after.chars().count();
                    tokens.push(MarkerToken::Operator("not in".into()));
                }
                _ => tokens.push(MarkerToken::Variable(word)),
            }
        } else {
            return Err(format!("has an unexpected marker character `{character}`"));
        }
    }
    Ok(tokens)
}

impl Marker {
    fn parse(text: &str) -> std::result::Result<Self, String> {
        let tokens = tokenize_marker(text)?;
        let mut position = 0;
        let marker = Self::parse_or(&tokens, &mut position)?;
        if position != tokens.len() {
            return Err("has trailing marker text".into());
        }
        Ok(marker)
    }

    fn parse_or(tokens: &[MarkerToken], position: &mut usize) -> std::result::Result<Self, String> {
        let mut left = Self::parse_and(tokens, position)?;
        while tokens.get(*position) == Some(&MarkerToken::Or) {
            *position += 1;
            let right = Self::parse_and(tokens, position)?;
            left = Self::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(
        tokens: &[MarkerToken],
        position: &mut usize,
    ) -> std::result::Result<Self, String> {
        let mut left = Self::parse_atom(tokens, position)?;
        while tokens.get(*position) == Some(&MarkerToken::And) {
            *position += 1;
            let right = Self::parse_atom(tokens, position)?;
            left = Self::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_atom(
        tokens: &[MarkerToken],
        position: &mut usize,
    ) -> std::result::Result<Self, String> {
        if tokens.get(*position) == Some(&MarkerToken::Open) {
            *position += 1;
            let inner = Self::parse_or(tokens, position)?;
            if tokens.get(*position) != Some(&MarkerToken::Close) {
                return Err("has an unbalanced marker parenthesis".into());
            }
            *position += 1;
            return Ok(inner);
        }
        let value = |token: Option<&MarkerToken>| match token {
            Some(MarkerToken::Variable(name)) => Ok(MarkerValue::Variable(name.clone())),
            Some(MarkerToken::Literal(text)) => Ok(MarkerValue::Literal(text.clone())),
            _ => Err("has an incomplete marker comparison".to_string()),
        };
        let left = value(tokens.get(*position))?;
        let Some(MarkerToken::Operator(operator)) = tokens.get(*position + 1) else {
            return Err("has a marker comparison without an operator".into());
        };
        let right = value(tokens.get(*position + 2))?;
        *position += 3;
        Ok(Self::Compare {
            left,
            operator: operator.clone(),
            right,
        })
    }

    /// Evaluate for `platform`, with `extra` the extra being requested (if
    /// any). Pumas installs no Python, so a comparison on a Python variable
    /// (`python_version`, `implementation_name`, ...) cannot exclude a
    /// library and counts as true.
    fn evaluate(&self, platform: WheelPlatform, extra: Option<&str>) -> bool {
        match self {
            Self::And(left, right) => {
                left.evaluate(platform, extra) && right.evaluate(platform, extra)
            }
            Self::Or(left, right) => {
                left.evaluate(platform, extra) || right.evaluate(platform, extra)
            }
            Self::Compare {
                left,
                operator,
                right,
            } => {
                let is_extra = |value: &MarkerValue| matches!(value, MarkerValue::Variable(name) if name == "extra");
                if is_extra(left) || is_extra(right) {
                    let other = if is_extra(left) { right } else { left };
                    let MarkerValue::Literal(named) = other else {
                        return false;
                    };
                    let equal = extra.is_some_and(|extra| normalize_name(named) == extra);
                    return match operator.as_str() {
                        "==" => equal,
                        "!=" => !equal,
                        _ => false,
                    };
                }
                let resolve = |value: &MarkerValue| match value {
                    MarkerValue::Literal(text) => Some(text.clone()),
                    MarkerValue::Variable(name) => platform.marker_value(name).map(str::to_owned),
                };
                let (Some(left), Some(right)) = (resolve(left), resolve(right)) else {
                    return true;
                };
                match operator.as_str() {
                    "==" | "===" => left == right,
                    "!=" => left != right,
                    "in" => right.contains(&left),
                    "not in" => !right.contains(&left),
                    other => match (
                        PyVersion::parse(&left),
                        Specifier::parse(&format!("{other}{right}")),
                    ) {
                        (Some(version), Some(specifier)) => specifier.contains(&version),
                        _ => false,
                    },
                }
            }
        }
    }
}

/// The `Requires-Dist` entries of a core metadata document.
pub(super) fn requires_dist(metadata: &str) -> std::result::Result<Vec<Requirement>, String> {
    let mut values: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in metadata.lines() {
        if line.is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) {
            if let Some(value) = current.as_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some(value) = current.take() {
            values.push(value);
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.trim().eq_ignore_ascii_case("requires-dist") {
                current = Some(value.trim().to_owned());
            }
        }
    }
    if let Some(value) = current {
        values.push(value);
    }
    values
        .iter()
        .map(|value| Requirement::parse(value))
        .collect()
}

/// A wheel file name (PEP 427): `name-version(-build)?-python-abi-platform.whl`.
#[derive(Clone, Debug)]
pub(super) struct WheelName {
    pub(super) project: String,
    pub(super) version: PyVersion,
    abi_tags: Vec<String>,
    platform_tags: Vec<String>,
}

impl WheelName {
    pub(super) fn parse(filename: &str) -> Option<Self> {
        let stem = filename.strip_suffix(".whl")?;
        let parts: Vec<&str> = stem.split('-').collect();
        let (name, version, abi, platform) = match parts.as_slice() {
            [name, version, _python, abi, platform] => (name, version, abi, platform),
            [name, version, _build, _python, abi, platform] => (name, version, abi, platform),
            _ => return None,
        };
        Some(Self {
            project: normalize_name(name),
            version: PyVersion::parse(version)?,
            abi_tags: abi.split('.').map(str::to_owned).collect(),
            platform_tags: platform.split('.').map(str::to_owned).collect(),
        })
    }

    /// Lower is better; `None` when the wheel cannot run on `platform`.
    fn rank(&self, platform: WheelPlatform) -> Option<(u8, u8)> {
        let platform_rank = self
            .platform_tags
            .iter()
            .filter_map(|tag| platform.tag_rank(tag))
            .min()?;
        let abi_rank = u8::from(!self.abi_tags.iter().any(|tag| tag == "none"));
        Some((platform_rank, abi_rank))
    }
}

/// One file a JSON simple index page lists.
#[derive(Clone, Debug)]
pub(super) struct IndexFile {
    pub(super) filename: String,
    pub(super) url: reqwest::Url,
    pub(super) sha256: Option<String>,
    pub(super) size: Option<u64>,
    pub(super) yanked: bool,
    /// `None`: no core metadata. `Some(None)`: metadata without a digest.
    pub(super) core_metadata: Option<Option<String>>,
    pub(super) wheel: Option<WheelName>,
}

fn sha256_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

/// What a package index answered for one project page.
#[derive(Clone)]
struct CachedPage {
    /// The [`IndexCache`] generation in which the index last confirmed it.
    generation: u64,
    etag: Option<String>,
    last_modified: Option<String>,
    /// `None`: the index does not have the project (404 or 410).
    files: Option<Vec<IndexFile>>,
}

/// Package index answers shared by version listings and installs.
///
/// A project page read in the current generation is used as it is. After
/// [`IndexCache::refresh`], the next read of a page revalidates it with the
/// index (`If-None-Match`, `If-Modified-Since`) and keeps it when the index
/// answers that it has not changed. Cached answers therefore last until the
/// index changes or a refresh is asked for, never for a fixed time. Core
/// metadata belongs to one immutable file and is kept for the process.
#[derive(Default)]
pub(crate) struct IndexCache {
    generation: AtomicU64,
    pages: StdMutex<HashMap<String, CachedPage>>,
    metadata: StdMutex<HashMap<String, String>>,
}

impl std::fmt::Debug for IndexCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IndexCache")
            .field("generation", &self.generation())
            .finish_non_exhaustive()
    }
}

impl IndexCache {
    /// Revalidate every cached page with its index on its next read.
    pub(crate) fn refresh(&self) {
        self.generation.fetch_add(1, AtomicOrdering::SeqCst);
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(AtomicOrdering::SeqCst)
    }

    fn page(&self, key: &str) -> Option<CachedPage> {
        self.pages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    fn store_page(&self, key: String, page: CachedPage) {
        self.pages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, page);
    }
}

/// How an index answered a project page request.
enum PageAnswer {
    NotModified,
    NotFound,
    Body {
        body: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}

/// A package index serving the JSON simple API (PEP 691).
#[derive(Clone, Debug)]
pub(super) struct PackageIndex {
    base: reqwest::Url,
    client: reqwest::Client,
    cache: Arc<IndexCache>,
}

fn header(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// Whether an index answered that a resource does not exist.
fn is_not_found(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::GONE
    )
}

impl PackageIndex {
    pub(super) fn new(base: &str, client: reqwest::Client, cache: Arc<IndexCache>) -> Result<Self> {
        let mut base = reqwest::Url::parse(base).map_err(|_| PumasError::Config {
            message: format!("Package index URL `{base}` is invalid"),
        })?;
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        Ok(Self {
            base,
            client,
            cache,
        })
    }

    /// The index URL without credentials, for messages and records.
    pub(super) fn display(&self) -> String {
        let mut shown = self.base.clone();
        let _ = shown.set_username("");
        let _ = shown.set_password(None);
        shown.set_query(None);
        shown.to_string()
    }

    fn failed(url: &reqwest::Url, cause: String) -> PumasError {
        PumasError::Network {
            message: format!("Could not read {url}"),
            cause: Some(cause),
        }
    }

    async fn read_body(
        url: &reqwest::Url,
        mut response: reqwest::Response,
        limit: usize,
    ) -> Result<Vec<u8>> {
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| Self::failed(url, error.without_url().to_string()))?
        {
            if body.len() + chunk.len() > limit {
                return Err(refused(format!(
                    "{url} is larger than the {limit} bytes Pumas reads from a package index"
                )));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    /// Request a project page, conditionally when `cached` has validators.
    async fn fetch_page(
        &self,
        url: &reqwest::Url,
        cached: Option<&CachedPage>,
    ) -> Result<PageAnswer> {
        let mut request = self
            .client
            .get(url.clone())
            .header(reqwest::header::ACCEPT, SIMPLE_JSON);
        if let Some(cached) = cached {
            if let Some(etag) = &cached.etag {
                request = request.header(reqwest::header::IF_NONE_MATCH, etag);
            }
            if let Some(modified) = &cached.last_modified {
                request = request.header(reqwest::header::IF_MODIFIED_SINCE, modified);
            }
        }
        let response = request
            .send()
            .await
            .map_err(|error| Self::failed(url, error.without_url().to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_MODIFIED && cached.is_some() {
            return Ok(PageAnswer::NotModified);
        }
        if is_not_found(status) {
            return Ok(PageAnswer::NotFound);
        }
        if !status.is_success() {
            return Err(Self::failed(url, format!("HTTP {status}")));
        }
        let content_type = header(&response, reqwest::header::CONTENT_TYPE).unwrap_or_default();
        if !content_type.contains(SIMPLE_JSON) {
            return Err(refused(format!(
                "Package index {} does not serve the JSON simple API (PEP 691); it answered \
                 `{content_type}`",
                self.display()
            )));
        }
        let etag = header(&response, reqwest::header::ETAG);
        let last_modified = header(&response, reqwest::header::LAST_MODIFIED);
        let body = Self::read_body(url, response, MAX_INDEX_PAGE_BYTES).await?;
        Ok(PageAnswer::Body {
            body,
            etag,
            last_modified,
        })
    }

    /// The files of `project`, or `None` when the index does not have it.
    pub(super) async fn files(&self, project: &str) -> Result<Option<Vec<IndexFile>>> {
        let page = self
            .base
            .join(&format!("{}/", normalize_name(project)))
            .map_err(|_| {
                refused(format!(
                    "Project name `{project}` is not a valid index path"
                ))
            })?;
        let key = page.to_string();
        let generation = self.cache.generation();
        let cached = self.cache.page(&key);
        if let Some(cached) = cached
            .as_ref()
            .filter(|cached| cached.generation == generation)
        {
            return Ok(cached.files.clone());
        }
        let (files, etag, last_modified) = match self.fetch_page(&page, cached.as_ref()).await? {
            PageAnswer::NotModified => {
                let cached = cached.expect("a page is only revalidated when cached");
                (cached.files, cached.etag, cached.last_modified)
            }
            PageAnswer::NotFound => (None, None, None),
            PageAnswer::Body {
                body,
                etag,
                last_modified,
            } => {
                let document: serde_json::Value =
                    serde_json::from_slice(&body).map_err(|error| {
                        refused(format!(
                            "Package index {} returned an unreadable page for {project}: {error}",
                            self.display()
                        ))
                    })?;
                let entries = document
                    .get("files")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| {
                        refused(format!(
                            "Package index {} returned no file list for {project}",
                            self.display()
                        ))
                    })?;
                let files = entries
                    .iter()
                    .filter_map(|entry| index_file(entry, &page, project))
                    .collect();
                (Some(files), etag, last_modified)
            }
        };
        self.cache.store_page(
            key,
            CachedPage {
                generation,
                etag,
                last_modified,
                files: files.clone(),
            },
        );
        Ok(files)
    }

    /// The core metadata served beside `file` (PEP 658), checked against the
    /// digest the index published for it.
    pub(super) async fn core_metadata(&self, file: &IndexFile) -> Result<String> {
        let Some(digest) = &file.core_metadata else {
            return Err(refused(format!(
                "Package index {} publishes no core metadata for {}, so Pumas cannot read its \
                 requirements without downloading it",
                self.display(),
                file.filename
            )));
        };
        let mut url = file.url.clone();
        url.set_fragment(None);
        let path = format!("{}.metadata", url.path());
        url.set_path(&path);
        let key = url.to_string();
        if let Some(text) = self
            .cache
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
        {
            return Ok(text.clone());
        }
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|error| Self::failed(&url, error.without_url().to_string()))?;
        if is_not_found(response.status()) {
            return Err(refused(format!(
                "Package index {} lists core metadata for {} but does not serve it",
                self.display(),
                file.filename
            )));
        }
        if !response.status().is_success() {
            return Err(Self::failed(&url, format!("HTTP {}", response.status())));
        }
        let body = Self::read_body(&url, response, MAX_METADATA_BYTES).await?;
        if let Some(expected) = digest {
            let actual = format!("{:x}", Sha256::digest(&body));
            if &actual != expected {
                return Err(refused(format!(
                    "Core metadata of {} has SHA-256 {actual}, not the {expected} its index \
                     published",
                    file.filename
                )));
            }
        }
        let text = String::from_utf8(body)
            .map_err(|_| refused(format!("Core metadata of {} is not UTF-8", file.filename)))?;
        self.cache
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, text.clone());
        Ok(text)
    }
}

/// One file entry of a PEP 691 JSON project page, or `None` when it names no
/// usable file.
fn index_file(entry: &serde_json::Value, page: &reqwest::Url, project: &str) -> Option<IndexFile> {
    let filename = entry.get("filename")?.as_str()?;
    let url = page.join(entry.get("url")?.as_str()?).ok()?;
    let metadata = entry
        .get("core-metadata")
        // PEP 714 renamed the PEP 691 field `dist-info-metadata`.
        .or_else(|| entry.get("dist-info-metadata"));
    let core_metadata = match metadata {
        None | Some(serde_json::Value::Bool(false)) | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Bool(true)) => Some(None),
        Some(value) => Some(sha256_value(value)),
    };
    let yanked = match entry.get("yanked") {
        Some(serde_json::Value::Bool(yanked)) => *yanked,
        Some(serde_json::Value::String(_)) => true,
        _ => false,
    };
    Some(IndexFile {
        filename: filename.to_owned(),
        sha256: entry.get("hashes").and_then(sha256_value),
        size: entry.get("size").and_then(serde_json::Value::as_u64),
        yanked,
        core_metadata,
        // A file of another project is not this project's wheel.
        wheel: WheelName::parse(filename).filter(|wheel| wheel.project == normalize_name(project)),
        url,
    })
}

/// The best wheel of `files` for `platform` with exactly `version`.
pub(super) fn best_wheel<'a>(
    files: &'a [IndexFile],
    version: &PyVersion,
    platform: WheelPlatform,
) -> Option<&'a IndexFile> {
    files
        .iter()
        .filter(|file| !file.yanked)
        .filter_map(|file| {
            let wheel = file.wheel.as_ref()?;
            // Prefer a wheel whose core metadata the index can verify.
            let metadata_rank: u8 = match &file.core_metadata {
                Some(Some(_)) => 0,
                Some(None) => 1,
                None => 2,
            };
            (&wheel.version == version)
                .then(|| wheel.rank(platform))
                .flatten()
                .map(|(platform_rank, abi_rank)| ((platform_rank, metadata_rank, abi_rank), file))
        })
        .min_by(|(left_rank, left), (right_rank, right)| {
            left_rank
                .cmp(right_rank)
                .then_with(|| right.filename.cmp(&left.filename))
        })
        .map(|(_, file)| file)
}

/// Final versions of `files` that have a wheel for `platform`, newest first.
pub(super) fn platform_versions(files: &[IndexFile], platform: WheelPlatform) -> Vec<PyVersion> {
    let mut versions: Vec<PyVersion> = files
        .iter()
        .filter(|file| !file.yanked)
        .filter_map(|file| file.wheel.as_ref())
        .filter(|wheel| wheel.version.is_final() && wheel.rank(platform).is_some())
        .map(|wheel| wheel.version.clone())
        .collect();
    versions.sort();
    versions.dedup();
    versions.reverse();
    versions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> PyVersion {
        PyVersion::parse(text).unwrap_or_else(|| panic!("{text} parses"))
    }

    #[test]
    fn versions_order_as_pep_440_and_only_finals_are_final() {
        let ordered = [
            "1.0.dev1",
            "1.0a1",
            "1.0b2",
            "1.0rc1",
            "1.0",
            "1.0.post1",
            "1.0.1",
            "9.28.0.42",
            "9.28.1.4",
            "13.5.37",
        ];
        for pair in ordered.windows(2) {
            assert!(version(pair[0]) < version(pair[1]), "{pair:?}");
        }
        assert_eq!(version("1.29"), version("1.29.0"));
        assert!(version("13.4.46rc1") < version("13.4.46"));
        assert!(!version("13.4.46rc1").is_final());
        assert!(!version("9.28.1.1.dev77966261").is_final());
        assert!(version("1.0.post2").is_final());
        assert!(version("12.9.79").is_final());
        assert!(PyVersion::parse("not-a-version").is_none());
    }

    #[test]
    fn specifiers_follow_pep_440() {
        let clause = |text: &str| Specifier::parse(text).unwrap();
        assert!(clause("~=13.0").contains(&version("13.5.37")));
        assert!(!clause("~=13.0").contains(&version("14.0")));
        assert!(!clause("~=13.0").contains(&version("12.9")));
        assert!(clause("~=9.0").contains(&version("9.28.1.4")));
        assert!(clause("==12.*").contains(&version("12.9.79")));
        assert!(!clause("==12.*").contains(&version("13.0")));
        assert!(clause("!=12.*").contains(&version("13.0")));
        assert!(clause("<10.4").contains(&version("10.3.9")));
        assert!(!clause(">1.0").contains(&version("1.0.post1")));
        assert!(clause("==1.29").contains(&version("1.29.0")));
        assert!(Specifier::parse("~=13").is_none());
        assert_eq!(clause("~=13.0").lower_bound().unwrap().major(), 13);
    }

    #[test]
    fn requirements_markers_and_extras_select_what_an_extra_adds() {
        let linux = WheelPlatform::LinuxX64;
        let extras: BTreeSet<String> = ["cuda".into(), "cudnn".into()].into();
        let metadata = "Metadata-Version: 2.1\nName: onnxruntime-gpu\nVersion: 1.31.1\n\
            Requires-Dist: numpy>=1.21.6\n\
            Requires-Dist: nvidia-cuda-runtime~=13.0; extra == \"cuda\"\n\
            Requires-Dist: nvidia-cudnn-cu13 (~=9.0) ; extra == 'cudnn'\n\
            Requires-Dist: sympy; extra == \"symbolic\"\n\
            Requires-Dist: nvidia-windows-only; extra == \"cuda\" and platform_system == \"Windows\"\n\
            Requires-Dist: nvidia-py; extra == \"cuda\" and python_version >= \"3.11\"\n\
            \n\
            Requires-Dist: in-the-body-not-a-header\n";
        let requirements = requires_dist(metadata).unwrap();
        let added: Vec<_> = requirements
            .iter()
            .filter(|requirement| requirement.added_by_extras(linux, &extras))
            .map(|requirement| requirement.name.as_str())
            .collect();
        assert_eq!(
            added,
            ["nvidia-cuda-runtime", "nvidia-cudnn-cu13", "nvidia-py"]
        );
        let windows: Vec<_> = requirements
            .iter()
            .filter(|requirement| requirement.added_by_extras(WheelPlatform::WindowsX64, &extras))
            .map(|requirement| requirement.name.as_str())
            .collect();
        assert!(windows.contains(&"nvidia-windows-only"));
        assert!(requirements[2].allows(&version("9.28.1.4")));
        assert!(Requirement::parse("pkg @ https://example.invalid/pkg.whl").is_err());
        let with_extras = Requirement::parse("Nvidia_CuBLAS[Extra_One]>=12,<13").unwrap();
        assert_eq!(with_extras.name, "nvidia-cublas");
        assert!(with_extras.extras.contains("extra-one"));
        assert!(with_extras.allows(&version("12.9")));
        assert!(!with_extras.allows(&version("13.0")));
    }

    #[test]
    fn wheel_names_and_platform_tags_pick_the_host_build() {
        let wheel = WheelName::parse(
            "nvidia_cuda_runtime-13.5.37-py3-none-manylinux2014_x86_64.manylinux_2_17_x86_64.whl",
        )
        .unwrap();
        assert_eq!(wheel.project, "nvidia-cuda-runtime");
        assert_eq!(wheel.version, version("13.5.37"));
        assert_eq!(wheel.rank(WheelPlatform::LinuxX64), Some((0, 0)));
        assert_eq!(wheel.rank(WheelPlatform::WindowsX64), None);
        let windows =
            WheelName::parse("nvidia_cudnn_cu12-9.28.1.4-py3-none-win_amd64.whl").unwrap();
        assert_eq!(windows.rank(WheelPlatform::WindowsX64), Some((0, 0)));
        assert!(
            WheelName::parse("nvidia_cudnn_cu12-9.28.1.4-py3-none-manylinux_2_27_aarch64.whl")
                .unwrap()
                .rank(WheelPlatform::LinuxX64)
                .is_none()
        );
        assert!(WheelName::parse("pkg-1.0.tar.gz").is_none());
    }

    #[test]
    fn shared_library_names_follow_the_platform_loader() {
        let linux = WheelPlatform::LinuxX64;
        for name in [
            "libcudart.so.12",
            "libcudnn.so.9",
            "libnvrtc-builtins.so.13.5",
            "libfoo.so",
        ] {
            assert!(linux.is_shared_library(name), "{name}");
        }
        for name in [
            "libcudart_static.a",
            "libcudart.so.12.txt",
            "cudart64_12.dll",
            "so.12",
        ] {
            assert!(!linux.is_shared_library(name), "{name}");
        }
        let windows = WheelPlatform::WindowsX64;
        assert!(windows.is_shared_library("cudnn_engines_precompiled64_9.dll"));
        assert!(windows.is_shared_library("CUDART64_13.DLL"));
        assert!(!windows.is_shared_library("cudart.lib"));
    }

    #[test]
    fn core_metadata_is_read_from_either_json_field_name() {
        let page = reqwest::Url::parse("https://index.test/simple/nvidia-cudnn-cu13/").unwrap();
        let digest = "a".repeat(64);
        let entry = |field: &str, value: serde_json::Value| {
            serde_json::json!({
                "filename": "nvidia_cudnn_cu13-9.1.0-py3-none-manylinux_2_27_x86_64.whl",
                "url": "../../files/nvidia_cudnn_cu13-9.1.0-py3-none-manylinux_2_27_x86_64.whl",
                "hashes": {"sha256": digest},
                field: value,
            })
        };
        for field in ["core-metadata", "dist-info-metadata"] {
            let file = index_file(
                &entry(field, serde_json::json!({"sha256": digest})),
                &page,
                "nvidia-cudnn-cu13",
            )
            .unwrap();
            assert_eq!(file.core_metadata, Some(Some(digest.clone())), "{field}");
            let file = index_file(
                &entry(field, serde_json::json!(false)),
                &page,
                "nvidia-cudnn-cu13",
            )
            .unwrap();
            assert_eq!(file.core_metadata, None, "{field}");
        }
        let no_name = serde_json::json!({"url": "x.whl"});
        assert!(index_file(&no_name, &page, "nvidia-cudnn-cu13").is_none());
    }

    #[test]
    fn linux_wheels_need_a_glibc_the_host_has() {
        let host = Some((2, 28));
        for tag in [
            "manylinux1_x86_64",
            "manylinux2014_x86_64",
            "manylinux_2_27_x86_64",
            "manylinux_2_28_x86_64",
        ] {
            assert!(linux_tag_supported(tag, host), "{tag}");
        }
        for tag in [
            "manylinux_2_34_x86_64",
            "manylinux_2_28_aarch64",
            "musllinux_1_2_x86_64",
            "linux_x86_64",
            "win_amd64",
        ] {
            assert!(!linux_tag_supported(tag, host), "{tag}");
        }
        assert!(linux_tag_supported("manylinux_2_34_x86_64", None));
        assert_eq!(
            manylinux_x86_64_glibc("manylinux2010_x86_64"),
            Some((2, 12))
        );
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        assert!(host_glibc().is_some_and(|(major, _)| major == 2));
    }

    #[test]
    fn project_names_normalize() {
        assert_eq!(normalize_name("Nvidia_CUDNN.cu12"), "nvidia-cudnn-cu12");
        assert_eq!(normalize_name("onnxruntime-gpu"), "onnxruntime-gpu");
    }
}
