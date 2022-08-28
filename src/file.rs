use anyhow::{bail, Result};
use chrono::prelude::*;
use log::info;
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug)]
pub enum FileSystemError {
    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    PersistError(#[from] tempfile::PersistError),
    #[error("File not found")]
    FileNotFound,

    #[error("{0}")]
    SerdeError(#[from] serde_json::Error),

    #[cfg(target_arch = "wasm32")]
    #[error("DOM exception")]
    DomException,
}

#[cfg(not(target_arch = "wasm32"))]
mod filesystem {
    use super::FileSystemError;
    use chrono::prelude::*;
    use std::fs;
    use std::path::Path;

    pub fn create_dir_all(dir: impl AsRef<Path>) -> Result<(), FileSystemError> {
        fs::create_dir_all(dir)?;
        Ok(())
    }

    pub async fn exists(path: &Path) -> Result<bool, FileSystemError> {
        Ok(path.is_file())
    }

    pub async fn write(
        path: impl AsRef<Path>,
        data: impl AsRef<[u8]>,
    ) -> Result<(), FileSystemError> {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new()?;
        f.write_all(data.as_ref())?;
        f.persist(path)?;
        Ok(())
    }

    pub async fn read(path: impl AsRef<Path>) -> Result<Vec<u8>, FileSystemError> {
        let ret = fs::read(path)?;
        Ok(ret)
    }

    pub async fn modified(path: impl AsRef<Path>) -> Result<DateTime<Local>, FileSystemError> {
        Ok(fs::metadata(path)?.modified()?.into())
    }
}

#[cfg(target_arch = "wasm32")]
mod filesystem {
    use super::FileSystemError;
    use chrono::prelude::*;
    use indexed_db_futures::prelude::*;
    use js_sys::Uint8Array;
    use log::info;
    use serde::{Deserialize, Serialize};
    use std::path::Path;
    use std::time::SystemTime;
    use wasm_bindgen::{prelude::*, JsCast};
    use web_sys::DomException;

    async fn open_db() -> Result<IdbDatabase, DomException> {
        let mut db_req: OpenDbRequest = IdbDatabase::open_u32("meru", 1)?;
        db_req.set_on_upgrade_needed(Some(|evt: &IdbVersionChangeEvent| -> Result<(), JsValue> {
            const STORES: &[&str] = &["save", "config", "data"];
            for store in STORES {
                if let None = evt.db().object_store_names().find(|n| n == store) {
                    evt.db().create_object_store(store)?;
                }
            }
            Ok(())
        }));

        let db = db_req.into_future().await?;
        Ok(db)
    }

    // parse path to (store name, file_name)
    fn parse_path(path: &Path) -> (String, String) {
        let mut it = path.iter();
        (
            it.next().unwrap().to_str().unwrap().to_string(),
            it.as_path().to_str().unwrap().to_string(),
        )
    }

    pub fn create_dir_all(_dir: impl AsRef<Path>) -> Result<(), FileSystemError> {
        Ok(())
    }

    pub async fn exists(path: &Path) -> Result<bool, FileSystemError> {
        let (store_name, file_name) = parse_path(path.as_ref());

        let db = open_db().await.map_err(|_| FileSystemError::DomException)?;

        let tx: IdbTransaction = db
            .transaction_on_one_with_mode(&store_name, IdbTransactionMode::Readonly)
            .map_err(|_| FileSystemError::DomException)?;

        let store: IdbObjectStore = tx
            .object_store(&store_name)
            .map_err(|_| FileSystemError::DomException)?;

        Ok(store
            .count_with_key_owned(&file_name)
            .map_err(|_| FileSystemError::DomException)?
            .await
            .map_err(|_| FileSystemError::DomException)?
            > 0)
    }

    #[derive(Serialize, Deserialize)]
    struct Metadata {
        modified: SystemTime,
    }

    pub async fn write(
        path: impl AsRef<Path>,
        data: impl AsRef<[u8]>,
    ) -> Result<(), FileSystemError> {
        info!("fs: write: {}", path.as_ref().display());

        let (store_name, file_name) = parse_path(path.as_ref());

        let db = open_db().await.map_err(|_| FileSystemError::DomException)?;

        let tx: IdbTransaction = db
            .transaction_on_one_with_mode(&store_name, IdbTransactionMode::Readwrite)
            .map_err(|_| FileSystemError::DomException)?;
        let store: IdbObjectStore = tx
            .object_store(&store_name)
            .map_err(|_| FileSystemError::DomException)?;

        store
            .put_key_val_owned(&file_name, &Uint8Array::from(data.as_ref()))
            .map_err(|_| FileSystemError::DomException)?;

        store
            .put_key_val_owned(
                &format!("{file_name}.metadata"),
                &JsValue::from_serde(&Metadata {
                    modified: Utc::now().into(),
                })?,
            )
            .map_err(|_| FileSystemError::DomException)?;

        tx.await
            .into_result()
            .map_err(|_| FileSystemError::DomException)?;

        Ok(())
    }

    pub async fn read(path: impl AsRef<Path>) -> Result<Vec<u8>, FileSystemError> {
        info!("fs: read: {}", path.as_ref().display());

        let (store_name, file_name) = parse_path(path.as_ref());

        let db = open_db().await.map_err(|_| FileSystemError::DomException)?;

        let tx: IdbTransaction = db
            .transaction_on_one_with_mode(&store_name, IdbTransactionMode::Readonly)
            .map_err(|_| FileSystemError::DomException)?;

        let store: IdbObjectStore = tx
            .object_store(&store_name)
            .map_err(|_| FileSystemError::DomException)?;

        let jsvalue = if let Some(jsvalue) = store
            .get_owned(file_name.as_str())
            .map_err(|_| FileSystemError::DomException)?
            .await
            .map_err(|_| FileSystemError::DomException)?
        {
            jsvalue
        } else {
            Err(FileSystemError::FileNotFound)?
        };

        let array = jsvalue
            .dyn_into::<Uint8Array>()
            .map_err(|_| FileSystemError::DomException)?;

        tx.await
            .into_result()
            .map_err(|_| FileSystemError::DomException)?;

        Ok(array.to_vec())
    }

    pub async fn modified(
        path: impl AsRef<Path>,
    ) -> anyhow::Result<DateTime<Local>, FileSystemError> {
        info!("fs: modified: {}", path.as_ref().display());

        let (store_name, file_name) = parse_path(path.as_ref());

        let db = open_db().await.map_err(|_| FileSystemError::DomException)?;

        let tx: IdbTransaction = db
            .transaction_on_one_with_mode(&store_name, IdbTransactionMode::Readwrite)
            .map_err(|_| FileSystemError::DomException)?;

        let store: IdbObjectStore = tx
            .object_store(&store_name)
            .map_err(|_| FileSystemError::DomException)?;

        let jsvalue = if let Some(jsvalue) = store
            .get_owned(&format!("{file_name}.metadata"))
            .map_err(|_| FileSystemError::DomException)?
            .await
            .map_err(|_| FileSystemError::DomException)?
        {
            jsvalue
        } else {
            Err(FileSystemError::FileNotFound)?
        };

        let metadata = jsvalue.into_serde::<Metadata>()?;

        tx.await
            .into_result()
            .map_err(|_| FileSystemError::DomException)?;

        Ok(metadata.modified.into())
    }
}

pub use filesystem::*;

pub async fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    info!("fs: read_to_string: {}", path.as_ref().display());

    let bin = read(path).await?;
    let ret = String::from_utf8(bin)?;
    Ok(ret)
}

pub fn get_save_dir(core_abbrev: &str, save_dir: &Path) -> Result<PathBuf> {
    let dir = save_dir.join(core_abbrev);

    if !dir.exists() {
        create_dir_all(&dir)?;
    } else if !dir.is_dir() {
        bail!("`{}` is not a directory", dir.display());
    }
    Ok(dir)
}

fn get_backup_file_path(core_abbrev: &str, name: &str, save_dir: &Path) -> Result<PathBuf> {
    Ok(get_save_dir(core_abbrev, save_dir)?.join(format!("{name}.sav")))
}

pub fn get_state_file_path(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    save_dir: &Path,
) -> Result<PathBuf> {
    Ok(get_save_dir(core_abbrev, save_dir)?.join(format!("{name}-{slot}.state")))
}

pub async fn load_backup(
    core_abbrev: &str,
    name: &str,
    save_dir: &Path,
) -> Result<Option<Vec<u8>>> {
    let path = get_backup_file_path(core_abbrev, name, save_dir)?;

    Ok(if exists(&path).await? {
        info!("Loading backup RAM: `{}`", path.display());
        Some(read(path).await?)
    } else {
        info!("Backup RAM not found: `{}`", path.display());
        None
    })
}

pub async fn save_backup(core_abbrev: &str, name: &str, ram: &[u8], save_dir: &Path) -> Result<()> {
    let path = get_backup_file_path(core_abbrev, name, save_dir)?;

    if !exists(&path).await? {
        info!("Creating backup RAM file: `{}`", path.display());
    } else {
        info!("Overwriting backup RAM file: `{}`", path.display());
    }
    write(&path, ram).await?;
    Ok(())
}

pub async fn save_state(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    data: &[u8],
    save_dir: &Path,
) -> Result<()> {
    write(
        &get_state_file_path(core_abbrev, name, slot, save_dir)?,
        data,
    )
    .await?;
    Ok(())
}

pub async fn load_state(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    save_dir: &Path,
) -> Result<Vec<u8>> {
    let ret = read(get_state_file_path(core_abbrev, name, slot, save_dir)?).await?;
    Ok(ret)
}

pub async fn state_date(
    core_abbrev: &str,
    name: &str,
    slot: usize,
    save_dir: &Path,
) -> Result<Option<DateTime<Local>>> {
    let path = get_state_file_path(core_abbrev, name, slot, save_dir)?;
    if let Ok(date) = modified(&path).await {
        Ok(Some(date))
    } else {
        Ok(None)
    }
}
