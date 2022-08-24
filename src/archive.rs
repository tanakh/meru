#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use anyhow::Result;
    use std::io::{Read, Seek, SeekFrom};

    pub trait ReadSeek: Read + Seek + Send + 'static {}
    impl<T: Read + Seek + Send + 'static> ReadSeek for T {}

    pub struct Archive {
        source: Box<dyn ReadSeek>,
    }

    impl Archive {
        pub fn new(source: impl ReadSeek) -> Result<Self> {
            Ok(Self {
                source: Box::new(source),
            })
        }

        pub fn file_names(&mut self) -> Result<Vec<String>> {
            let ret = compress_tools::list_archive_files(&mut self.source)?;
            Ok(ret)
        }

        pub fn uncompress_file(&mut self, path: &str) -> Result<Vec<u8>> {
            let mut data = vec![];
            self.source.seek(SeekFrom::Start(0))?;
            compress_tools::uncompress_archive_file(&mut self.source, &mut data, path)?;
            Ok(data)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod inner {
    use anyhow::Result;
    use std::io::{Read, Seek};
    use zip::ZipArchive;

    pub trait ReadSeek: Read + Seek + Send + 'static {}
    impl<T: Read + Seek + Send + 'static> ReadSeek for T {}

    pub struct Archive {
        zip: ZipArchive<Box<dyn ReadSeek>>,
    }

    impl Archive {
        pub fn new(reader: impl ReadSeek) -> Result<Self> {
            let reader = Box::new(reader) as Box<dyn ReadSeek>;
            let zip = ZipArchive::new(reader)?;
            Ok(Self { zip })
        }

        pub fn file_names(&mut self) -> Result<Vec<String>> {
            let ret = self
                .zip
                .file_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            Ok(ret)
        }

        pub fn uncompress_file(&mut self, path: &str) -> Result<Vec<u8>> {
            let mut file = self.zip.by_name(path)?;
            let mut data = vec![];
            file.read_to_end(&mut data)?;
            Ok(data)
        }
    }
}

pub use inner::*;
