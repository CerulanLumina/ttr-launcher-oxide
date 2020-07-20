use crate::opt::Options;

pub use error::*;
use manifest::Manifest;
use crate::update::manifest::{FileObject, PatchObject};
use std::path::{Path, PathBuf};
use sha::utils::{Digest, DigestExt};
use futures::Future;
use std::fs::DirBuilder;
use sha::sha1::Sha1;
use std::io::Write;
use std::fs::File;

const MANIFEST_URL: &'static str = "https://cdn.toontownrewritten.com/content/patchmanifest.txt";
const CDN_BASE_URL: &'static str = "https://download.toontownrewritten.com/patches/";

#[cfg(target_os = "linux")]
const PLATFORM_KEY: &'static str = "linux2";
#[cfg(target_os = "macos")]
const PLATFORM_KEY: &'static str = "darwin";
#[cfg(all(windows, target_arch = "x86_64"))]
const PLATFORM_KEY: &'static str = "win64";
#[cfg(all(windows, target_arch = "x86"))]
const PLATFORM_KEY: &'static str = "win32";

pub async fn update(options: &Options) -> Result<(), UpdateError> {
    if !options.install_dir.exists() {
        DirBuilder::new().recursive(true).create(&options.install_dir)?;
    }
    let platform_key_string = String::from(PLATFORM_KEY);
    let manifest: Manifest = fetch_manifest().await?;
    let handle = tokio::runtime::Handle::current();
    let threads = manifest.into_iter()
        .filter(|a| a.1.only.contains(&platform_key_string))
        .map(|a| update_file(options.install_dir.clone(), a.0, a.1))
        .map(|fut| handle.spawn(fut));
    join_updaters(threads).await;
    if let Err(err) = set_executable(options.install_dir.join("TTREngine")).await  {
        eprintln!("Failed to set executable flag!\n{}", err);
        Err(err)
    } else {
        Ok(())
    }
}
#[cfg(unix)]
async fn set_executable(engine: PathBuf) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = async_std::fs::metadata(&engine).await?.permissions();
    let mut mode: u32 = perms.mode();
    mode |= 0o0500;
    perms.set_mode(mode);
    async_std::fs::set_permissions(&engine, perms).await?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_: PathBuf) -> Result<(), UpdateError> { Ok(()) }

async fn join_updaters<I>(i: I) where
    I: IntoIterator,
    <I as IntoIterator>::Item: Future {

    futures::future::join_all(i).await;

}

async fn update_file(dir: PathBuf, filename: String, obj: FileObject) -> Result<(), UpdateError> {
    let path = dir.join(&filename);
    if path.exists() {
        let existing = async_std::fs::read(&path).await?;
        let hash = sha::sha1::Sha1::default().digest(&existing).to_hex();
        if obj.hash == hash {
            // already up to date
            Ok(())
        } else {
            // not up to date
            // check available patches
            // match obj.patches.get(&hash) {
            //     Some(patch) => patch_file(&path, &obj,patch).await,
            //     None => download_fresh(&path, &obj).await,
            // }
            // TODO setup patching
            download_fresh(&path, &obj).await
        }
    } else {
        download_fresh(&path, &obj).await
    }
}

async fn download_fresh(file_path: &Path, obj: &FileObject) -> Result<(), UpdateError> {
    let url = format!("{}{}", CDN_BASE_URL, &obj.dl);
    let bytes = reqwest::get(&url).await?.bytes().await?.to_vec();
    let dl_sha = sha1(&bytes).to_hex();
    if dl_sha == obj.comp_hash {
        let mut bzd = bzip2::write::BzDecoder::new(File::create(file_path)?);
        bzd.write_all(bytes.as_slice())?;
        println!("Downloaded {:?}", file_path.file_name().unwrap());
        Ok(())
    } else {
        Err(UpdateError::Patching)
    }
}

// TODO implement patchinng
#[allow(unused)]
async fn patch_file(file_path: &Path, file_object: &FileObject, patch_object: &PatchObject) -> Result<(), UpdateError> {
    unimplemented!()
}

async fn fetch_manifest() -> Result<Manifest, UpdateError> {
    let resp = reqwest::get(MANIFEST_URL).await?;
    let text = resp.text().await?;
    let m: Manifest = serde_json::from_str(text.as_str())?;
    Ok(m)
}

mod manifest {
    use serde::Deserialize;
    use std::collections::HashMap;

    pub type Manifest = HashMap<String, FileObject>;
    pub type PatchesObject = HashMap<String, PatchObject>;

    #[derive(Deserialize)]
    pub struct FileObject {
        pub dl: String,
        pub only: Vec<String>,
        pub hash: String,
        #[serde(rename = "compHash")]
        pub comp_hash: String,
        pub patches: PatchesObject,
    }

    #[derive(Deserialize)]
    pub struct PatchObject {
        pub filename: String,
        #[serde(rename = "compPatchHash")]
        pub comp_patch_hash: String,
        #[serde(rename = "patchHash")]
        pub patch_hash: String,
    }
}

mod error {
    use std::fmt::{Debug, Result as FmtResult, Formatter};
    use reqwest::Error;

    #[derive(Debug)]
    pub enum UpdateError {
        Downloading(reqwest::Error),
        Parsing(serde_json::Error),
        IO(std::io::Error),
        Patching,
    }

    impl std::error::Error for UpdateError {}
    impl std::fmt::Display for UpdateError {
        fn fmt(&self, f: &mut Formatter) -> FmtResult {
            match self {
                // TODO add more information
                Self::Downloading(inner) => write!(f, "Error occurred while downloading a file: {}", inner),
                Self::Parsing(inner) => write!(f, "The web response was malformed: {}", inner),
                Self::IO(inner) => write!(f, "An IO error occurred: {}", inner),
                Self::Patching => write!(f, "Error occurred while patching a file"),
            }
        }
    }

    impl From<reqwest::Error> for UpdateError {
        fn from(err: Error) -> Self {
            if err.is_builder() { panic!("The request API was incorrectly called! This is a bug!"); }
            Self::Downloading(err)
        }
    }

    impl From<serde_json::Error> for UpdateError {
        fn from(err: serde_json::Error) -> Self {
            Self::Parsing(err)
        }
    }

    impl From<std::io::Error> for UpdateError {
        fn from(err: std::io::Error) -> Self {
            Self::IO(err)
        }
    }
}

fn sha1<D: AsRef<[u8]>>(data: D) -> Sha1 {
    let mut sha = Sha1::default();
    sha.digest(data.as_ref());
    sha
}
