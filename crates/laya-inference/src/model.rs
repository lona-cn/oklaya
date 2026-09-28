//! Pinned Laya ONNX artifacts and verified, resumable local cache.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use sha2::{Digest, Sha256};

use crate::Error;

pub const REVISION: &str = "1bc2622b5a4e4ceb46aadf709d7a360eb7d3d1f4";
const BASE_URL: &str = "https://huggingface.co/codenamev/laya-onnx/resolve";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelKind {
    English,
    Multilingual,
    TypedDecisions,
}

impl ModelKind {
    pub const ALL: [Self; 3] = [Self::English, Self::Multilingual, Self::TypedDecisions];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::English => "english",
            Self::Multilingual => "multilingual",
            Self::TypedDecisions => "typed-decisions",
        }
    }

    fn assets(self) -> &'static [Asset; 5] {
        match self {
            Self::English => &ENGLISH,
            Self::Multilingual => &MULTILINGUAL,
            Self::TypedDecisions => &TYPED_DECISIONS,
        }
    }
}

impl fmt::Display for ModelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ModelKind {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "english" => Ok(Self::English),
            "multilingual" => Ok(Self::Multilingual),
            "typed-decisions" => Ok(Self::TypedDecisions),
            _ => Err(Error::InvalidModelInput(format!(
                "unknown model {value:?}; choose english, multilingual, or typed-decisions"
            ))),
        }
    }
}

#[derive(Clone, Copy)]
struct Asset {
    name: &'static str,
    size: u64,
    sha256: &'static str,
}

const ENGLISH: [Asset; 5] = [
    Asset {
        name: "model.onnx",
        size: 846075711,
        sha256: "ea2a37ef3bea4c1ebf7484b3b3da51de51cf9949fdd8a0088129db10dbb83615",
    },
    Asset {
        name: "rl_agent_config.json",
        size: 745,
        sha256: "ae287b56bbcf5f8c4f4541ae9dfd00c914c4c48b940b8398c3058af37ba92bbd",
    },
    Asset {
        name: "onnx_config.json",
        size: 424,
        sha256: "8c799c013cb8f0881d33a533dd9539fba7784231ea5746803cd69f1321132240",
    },
    Asset {
        name: "tokenizer/tokenizer.json",
        size: 3583228,
        sha256: "6c8aaa9a542084f2457eab775d4eeb51f92a70c0fd9de28d5edb0ddec3c08d30",
    },
    Asset {
        name: "tokenizer/tokenizer_config.json",
        size: 308,
        sha256: "50044de60daaa73df97d262e15a40d4faf0160e7d742df64b377877a1320dd12",
    },
];
const MULTILINGUAL: [Asset; 5] = [
    Asset {
        name: "model.onnx",
        size: 646693695,
        sha256: "da6a0f87380597f679b12ce539e0a187e923fcffcf8dfc2f035728388dceacb0",
    },
    Asset {
        name: "rl_agent_config.json",
        size: 472,
        sha256: "25061739243b617ad88d1219ba6f8a9c86c5881ca28df024fa2d9b3b2fcc30c6",
    },
    Asset {
        name: "onnx_config.json",
        size: 428,
        sha256: "7eff4d0af9a8b22b977b690ab9e7d97ea2cda6c0a449e53502c06f7db2ae915f",
    },
    Asset {
        name: "tokenizer/tokenizer.json",
        size: 34363188,
        sha256: "609d8f4c067cd3950f88594c5a802616cea245823836ef5848ee4fc40aab5b6f",
    },
    Asset {
        name: "tokenizer/tokenizer_config.json",
        size: 524,
        sha256: "6c6b2d8e3c84ce0e671c129cd6b374b235d6f9863042a5836358d00a89bbb5a1",
    },
];
const TYPED_DECISIONS: [Asset; 5] = [
    Asset {
        name: "model.onnx",
        size: 846075712,
        sha256: "7bc0662f05f69635291b1f823a946931b9936fcebd2ea98db85da70ebb8f4ba2",
    },
    Asset {
        name: "rl_agent_config.json",
        size: 847,
        sha256: "ebf0cd524d92342a6be5e48e9fca3d7c2babfb5a56ccd79d2171ef5d8c7f7be8",
    },
    Asset {
        name: "onnx_config.json",
        size: 429,
        sha256: "b9a3e4dff6c65c5aaaaa310e0cc679098935ea151d53cd02f05c86b347c57518",
    },
    Asset {
        name: "tokenizer/tokenizer.json",
        size: 3583228,
        sha256: "6c8aaa9a542084f2457eab775d4eeb51f92a70c0fd9de28d5edb0ddec3c08d30",
    },
    Asset {
        name: "tokenizer/tokenizer_config.json",
        size: 337,
        sha256: "08d4cf3ac4dca381759441b85b91a6d40e688471dcd33d15d6649eb0a9a854d1",
    },
];

/// The revision-specific, platform-standard cache directory for this model.
pub fn model_dir(kind: ModelKind) -> Result<PathBuf, Error> {
    let dirs = directories::ProjectDirs::from("dev", "", "laya-rs").ok_or_else(|| {
        Error::ModelDownload("cannot determine the system cache directory".into())
    })?;
    Ok(dirs.cache_dir().join(REVISION).join(kind.as_str()))
}

/// Resolve an installed model; fail rather than using an incomplete or corrupted cache.
pub fn path(kind: ModelKind) -> Result<PathBuf, Error> {
    path_in(kind, &model_dir(kind)?)
}

/// Resolve and verify an installed model in an application-selected directory.
pub fn path_in(kind: ModelKind, dir: &Path) -> Result<PathBuf, Error> {
    for asset in kind.assets() {
        let file = dir.join(asset.name);
        if !file.is_file() {
            return Err(Error::InvalidModelInput(format!(
                "{kind} is not downloaded: {} is missing",
                asset.name
            )));
        }
        if !verify(&file, asset).map_err(download_error)? {
            return Err(Error::ChecksumMismatch(format!(
                "{} does not match the pinned {kind} artifact",
                file.display()
            )));
        }
    }
    Ok(dir.join("model.onnx"))
}

/// Return whether each complete model has passed size and SHA256 checks.
pub fn list() -> Result<Vec<(ModelKind, bool)>, Error> {
    ModelKind::ALL
        .into_iter()
        .map(|kind| {
            let dir = model_dir(kind)?;
            let mut valid = true;
            for asset in kind.assets() {
                if !verify(&dir.join(asset.name), asset).map_err(download_error)? {
                    valid = false;
                    break;
                }
            }
            Ok((kind, valid))
        })
        .collect()
}

/// Download all missing or invalid artifacts; interrupted files remain resumable.
pub fn download(kind: ModelKind) -> Result<PathBuf, Error> {
    download_to(kind, &model_dir(kind)?)
}

/// Download and verify a pinned model in an application-selected directory.
pub fn download_to(kind: ModelKind, dir: &Path) -> Result<PathBuf, Error> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(download_error)?;
    for asset in kind.assets() {
        let file = dir.join(asset.name);
        if verify(&file, asset).map_err(download_error)? {
            continue;
        }
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(download_error)?;
        }
        fetch(&client, kind, asset, &file)?;
    }
    Ok(dir.join("model.onnx"))
}

fn download_error(error: impl fmt::Display) -> Error {
    Error::ModelDownload(error.to_string())
}

fn verify(file: &Path, asset: &Asset) -> std::io::Result<bool> {
    let mut file = match File::open(file) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() != asset.size {
        return Ok(false);
    }
    let mut hash = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buf)?;
        if count == 0 {
            break;
        }
        hash.update(&buf[..count]);
    }
    Ok(hex::encode(hash.finalize()) == asset.sha256)
}

fn fetch(client: &Client, kind: ModelKind, asset: &Asset, destination: &Path) -> Result<(), Error> {
    let part = destination.with_extension(format!(
        "{}part",
        destination
            .extension()
            .and_then(|s| s.to_str())
            .map_or(String::new(), |s| format!("{s}."))
    ));
    let mut output = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&part)
        .map_err(download_error)?;
    let mut offset = output.metadata().map_err(download_error)?.len();
    if offset > asset.size
        || (offset == asset.size && !verify(&part, asset).map_err(download_error)?)
    {
        output.set_len(0).map_err(download_error)?;
        offset = 0;
    }
    if offset < asset.size {
        let url = format!("{BASE_URL}/{REVISION}/{kind}/{}?download=true", asset.name);
        let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
        if offset > 0 {
            request = request.header(RANGE, format!("bytes={offset}-"));
        }
        let mut response = request
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(download_error)?;
        match response.status() {
            StatusCode::OK => {
                if offset != 0 {
                    output.set_len(0).map_err(download_error)?;
                    offset = 0;
                }
                check_length(&response, asset.size)?;
            }
            StatusCode::PARTIAL_CONTENT if offset > 0 => {
                let value = response
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| {
                        download_error(format!("missing Content-Range for {kind}/{}", asset.name))
                    })?;
                let expected = format!("bytes {offset}-{}/{}", asset.size - 1, asset.size);
                if value != expected {
                    return Err(download_error(format!(
                        "unexpected Content-Range {value:?} for {kind}/{} (expected {expected})",
                        asset.name
                    )));
                }
                check_length(&response, asset.size - offset)?;
            }
            status => {
                return Err(download_error(format!(
                    "unexpected HTTP {status} for {kind}/{}",
                    asset.name
                )));
            }
        }
        output
            .seek(SeekFrom::Start(offset))
            .map_err(download_error)?;
        let progress = ProgressBar::new(asset.size);
        progress.set_draw_target(ProgressDrawTarget::stderr());
        progress.set_style(
            ProgressStyle::with_template(
                "{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
            )
            .expect("valid progress template"),
        );
        progress.set_message(format!("{kind}/{}", asset.name));
        progress.set_position(offset);
        let mut received = offset;
        let mut buffer = [0u8; 64 * 1024];
        let result = (|| -> Result<(), Error> {
            loop {
                let n = response.read(&mut buffer).map_err(download_error)?;
                if n == 0 {
                    break;
                }
                received = received
                    .checked_add(n as u64)
                    .ok_or_else(|| download_error("download size overflow"))?;
                if received > asset.size {
                    return Err(download_error(format!(
                        "{} exceeded pinned size {}",
                        asset.name, asset.size
                    )));
                }
                output.write_all(&buffer[..n]).map_err(download_error)?;
                progress.set_position(received);
            }
            output.flush().map_err(download_error)?;
            if received != asset.size {
                return Err(download_error(format!(
                    "incomplete {kind}/{}: received {received} of {} bytes",
                    asset.name, asset.size
                )));
            }
            Ok(())
        })();
        progress.finish_and_clear();
        result?;
    }
    output.sync_all().map_err(download_error)?;
    drop(output);
    if !verify(&part, asset).map_err(download_error)? {
        fs::remove_file(&part).map_err(download_error)?;
        return Err(Error::ChecksumMismatch(format!(
            "{kind}/{} failed pinned SHA256 verification",
            asset.name
        )));
    }
    if destination.exists() {
        fs::remove_file(destination).map_err(download_error)?;
    }
    fs::rename(&part, destination).map_err(download_error)?;
    Ok(())
}

fn check_length(response: &reqwest::blocking::Response, expected: u64) -> Result<(), Error> {
    if let Some(value) = response.headers().get(CONTENT_LENGTH) {
        let length = value.to_str().ok().and_then(|s| s.parse::<u64>().ok());
        if length != Some(expected) {
            return Err(download_error(format!(
                "unexpected Content-Length {value:?}; expected {expected}"
            )));
        }
    }
    Ok(())
}
