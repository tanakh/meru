use schemars::{
    gen::SchemaGenerator,
    schema::{Schema, SchemaObject},
    JsonSchema,
};
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    #[derive(Clone, Serialize, Deserialize)]
    #[serde(from = "String", into = "String")]
    pub struct File {
        pub(crate) path: PathBuf,
    }

    impl From<String> for File {
        fn from(s: String) -> Self {
            File {
                path: PathBuf::from(s),
            }
        }
    }

    impl From<File> for String {
        fn from(f: File) -> Self {
            f.path.to_string_lossy().to_string()
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use base64::STANDARD;
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    #[derive(Clone, Serialize, Deserialize)]
    pub struct File {
        pub(crate) path: PathBuf,
        #[serde(with = "Base64Standard")]
        pub(crate) data: Vec<u8>,
    }

    base64_serde_type!(Base64Standard, STANDARD);
}

pub use imp::File;

impl JsonSchema for File {
    fn schema_name() -> String {
        "File".to_string()
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        let mut schema: SchemaObject = <String>::json_schema(gen).into();
        schema.format = Some("file".to_owned());
        schema.into()
    }
}

impl File {
    #[allow(unused_variables)]
    pub fn new(path: PathBuf, data: Vec<u8>) -> Self {
        File {
            path,
            #[cfg(target_arch = "wasm32")]
            data,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn data(&self) -> Result<Vec<u8>, std::io::Error> {
        std::fs::read(&self.path)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn data(&self) -> Result<Vec<u8>, std::io::Error> {
        Ok(self.data.clone())
    }
}
