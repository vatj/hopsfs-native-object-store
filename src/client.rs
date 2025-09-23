use libc::{O_RDONLY, O_WRONLY};
use std::ffi::{CStr, CString};

use crate::native;
use crate::native::{hdfsFS, hdfsFile, hdfsFileInfo, tObjectKind, tOffset, tSize};
use bytes::Bytes;
use futures::stream::Stream;
use libc::{c_int, c_short, c_ushort, c_void, int32_t};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tokio::task;

const DATA_BLOCK_SIZE: usize = 65536;
const MAX_CONNECTIONS: usize = 1;

#[derive(Error, Debug)]
pub enum HdfsError {
    #[error("File not found at {0}")]
    FileNotFound(String),
    #[error("File already exists at {0}")]
    AlreadyExists(String),
    #[error("Invalid path to {0}")]
    InvalidPath(String),
    #[error("{0}")]
    NoneUnicodeInPath(String),
    #[error("{0} is a directory")]
    IsADirectoryError(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Invalid connection URL {0}")]
    InvalidUri(String),
    #[error("{0} is not supported yet!")]
    NotSupported(String),
}

pub type Result<T> = std::result::Result<T, HdfsError>;

#[derive(Clone)]
pub struct WriteOptions {
    pub block_size: Option<c_int>,
    pub replication: Option<c_short>,
    pub overwrite: bool,
    pub create_parent: bool, //create_parent false is not supported, yet!
    pub buffer_size: c_int,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            block_size: None,
            replication: None,
            overwrite: false,
            create_parent: true,
            buffer_size: 0,
        }
    }
}

#[derive(Debug)]
pub struct FileStatus {
    pub path: String,
    pub length: usize,
    pub isdir: bool,
    pub permission: u16,
    pub owner: String,
    pub group: String,
    pub modification_time: u64,
    pub access_time: u64,
    pub replication: Option<u32>,
    pub blocksize: Option<u64>,
}

impl FileStatus {
    pub fn from_hdfs_file_info(file_info: *const hdfsFileInfo) -> Result<Self> {
        unsafe {
            let path = CStr::from_ptr((*file_info).mName)
                .to_str()
                .ok()
                .map(|s| s.to_owned());

            let owner = CStr::from_ptr((*file_info).mOwner)
                .to_str()
                .ok()
                .map(|s| s.to_owned());

            let group = CStr::from_ptr((*file_info).mGroup)
                .to_str()
                .ok()
                .map(|s| s.to_owned());

            Ok(FileStatus {
                path: path.unwrap(),
                length: (*file_info).mSize as usize,
                isdir: (*file_info).mKind == tObjectKind::kObjectKindDirectory,
                permission: (*file_info).mPermissions as u16,
                owner: owner.unwrap(),
                group: group.unwrap(),
                modification_time: (*file_info).mLastMod as u64,
                access_time: (*file_info).mLastAccess as u64,
                replication: Some((*file_info).mReplication as u32),
                blocksize: Some((*file_info).mBlockSize as u64),
            })
        }
    }
}

#[derive(Debug)]
pub struct FileReader {
    connection: Arc<Connection>,
    file: Arc<AtomicPtr<hdfsFile>>,
    closed: AtomicBool,
}

impl FileReader {
    pub fn new(connection: Arc<Connection>, file: *mut hdfsFile) -> Self {
        FileReader {
            connection,
            file: Arc::new(AtomicPtr::new(file)),
            closed: AtomicBool::new(false),
        }
    }
    pub fn get_file_ptr(&self) -> *const hdfsFile {
        self.file.load(Ordering::SeqCst)
    }

    pub fn get_file_handle(&self) -> Arc<AtomicPtr<hdfsFile>> {
        Arc::clone(&self.file)
    }

    pub async fn hdfs_read(&self, buffer_size: usize) -> Result<Bytes> {
        let file_ptr = self.get_file_ptr() as usize;
        let connection = Arc::clone(&self.connection);
        
        let res = task::spawn_blocking(move || {
            let mut buffer = vec![0u8; buffer_size];
            let buf_ptr = buffer.as_mut_ptr().cast::<c_void>();
            let buf_len = buffer_size as tSize;
            
            unsafe {
                let bytes_read = native::hdfsRead(
                    connection.get_conn_ptr(),
                    file_ptr as *const hdfsFile,
                    buf_ptr,
                    buf_len,
                );
                
                if bytes_read < 0 {
                    Err(HdfsError::OperationFailed(
                        "File read operation failed".to_string(),
                    ))
                } else if bytes_read == 0 {
                    // End of file
                    Ok(Bytes::new())
                } else {
                    // Truncate buffer to actual bytes read
                    buffer.truncate(bytes_read as usize);
                    Ok(Bytes::from(buffer))
                }
            }
        })
        .await;

        match res {
            Ok(result) => result,
            Err(_) => Err(HdfsError::OperationFailed(
                "File read operation failed".to_string(),
            )),
        }
    }

    pub async fn hdfs_seek(&self, position: i64) -> Result<()> {
        let file_ptr = self.get_file_ptr() as usize;
        let connection = Arc::clone(&self.connection);
        
        let res = task::spawn_blocking(move || unsafe {
            native::hdfsSeek(
                connection.get_conn_ptr(),
                file_ptr as *const hdfsFile,
                position as tOffset,
            )
        })
        .await;

        if res.is_err() || res.unwrap() == -1 {
            Err(HdfsError::OperationFailed(
                "File seek operation failed".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    pub async fn hdfs_tell(&self) -> Result<i64> {
        let file_ptr = self.get_file_ptr() as usize;
        let connection = Arc::clone(&self.connection);
        
        let res = task::spawn_blocking(move || unsafe {
            native::hdfsTell(
                connection.get_conn_ptr(),
                file_ptr as *const hdfsFile,
            )
        })
        .await;

        match res {
            Ok(position) => {
                if position == -1 {
                    Err(HdfsError::OperationFailed(
                        "File tell operation failed".to_string(),
                    ))
                } else {
                    Ok(position)
                }
            },
            Err(_) => Err(HdfsError::OperationFailed(
                "File tell operation failed".to_string(),
            )),
        }
    }

    pub async fn close_file(&self) -> Result<()> {
        let file_ptr = self.get_file_ptr() as usize;
        let connection = Arc::clone(&self.connection);
        let res = task::spawn_blocking(move || unsafe {
            native::hdfsCloseFile(connection.get_conn_ptr(), file_ptr as *const hdfsFile)
        })
        .await;

        if res.is_err() || res.unwrap() == -1 {
            return Err(HdfsError::OperationFailed(
                "File close operation failed".to_string(),
            ));
        }
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for FileReader {
    fn drop(&mut self) {
        if !self.closed.load(Ordering::SeqCst) {
            unsafe {
                native::hdfsCloseFile(self.connection.get_conn_ptr(), self.get_file_ptr());
            }
        }
    }
}

#[derive(Debug)]
pub struct FileWriter {
    connection: Arc<Connection>,
    file: Arc<AtomicPtr<hdfsFile>>,
    closed: AtomicBool,
}

impl FileWriter {
    pub fn new(connection: Arc<Connection>, file: *mut hdfsFile) -> Self {
        FileWriter {
            connection,
            file: Arc::new(AtomicPtr::new(file)),
            closed: AtomicBool::new(false),
        }
    }
    pub fn get_file_ptr(&self) -> *const hdfsFile {
        self.file.load(Ordering::SeqCst)
    }

    pub async fn hdfs_write(&self, buf: Bytes) -> Result<()> {
        let file_ptr = self.get_file_ptr() as usize;
        let connection = Arc::clone(&self.connection);
        let res = task::spawn_blocking(move || {
            let buf_ptr = buf.as_ptr().cast::<c_void>();
            let buf_len = buf.len() as tSize;
            unsafe {
                native::hdfsWrite(
                    connection.get_conn_ptr(),
                    file_ptr as *const hdfsFile,
                    buf_ptr,
                    buf_len,
                )
            }
        })
        .await;

        if res.is_err() || res.unwrap() == -1 {
            Err(HdfsError::OperationFailed(
                "File write operation failed".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    pub async fn close_file(&self) -> Result<()> {
        let file_ptr = self.get_file_ptr() as usize;
        let connection = Arc::clone(&self.connection);
        let res = task::spawn_blocking(move || unsafe {
            native::hdfsCloseFile(connection.get_conn_ptr(), file_ptr as *const hdfsFile)
        })
        .await;

        if res.is_err() || res.unwrap() == -1 {
            return Err(HdfsError::OperationFailed(
                "File close operation failed".to_string(),
            ));
        }
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for FileWriter {
    fn drop(&mut self) {
        if !self.closed.load(Ordering::SeqCst) {
            unsafe {
                native::hdfsCloseFile(self.connection.get_conn_ptr(), self.get_file_ptr());
            }
        }
    }
}

#[derive(Debug)]
pub struct Connection {
    pub ptr: AtomicPtr<hdfsFS>,
}

impl Connection {
    pub fn new(ptr: *const hdfsFS) -> Self {
        Connection {
            ptr: AtomicPtr::new(ptr.cast_mut()),
        }
    }

    fn get_conn_ptr(&self) -> *const hdfsFS {
        self.ptr.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct HopsClient {
    pub hdfs_internal: Arc<Vec<Arc<Connection>>>,
    next_conn_idx: AtomicUsize,
}

impl Drop for HopsClient {
    /// Disconnect from the HDFS filesystem.
    /// This can potentially cause problem if the disconnect fails.
    /// Yet there is no explicit close process exists in the lib.rs
    fn drop(&mut self) {
        for i in 0..MAX_CONNECTIONS {
            unsafe {
                let ret = native::hdfsDisconnect(self.hdfs_internal[i].get_conn_ptr());
                if ret != 0 {
                    eprintln!("hdfsDisconnect failed with error code: {}", ret);
                }
            }
        }
    }
}

impl HopsClient {
    pub fn with_url(url: &str) -> Result<Self> {
        let mut connections = Vec::with_capacity(MAX_CONNECTIONS);
        for _ in 0..MAX_CONNECTIONS {
            let fs = Self::hopsfs_connect_with_url(url)?;
            let connection = Arc::new(Connection::new(fs));
            connections.push(connection);
        }
        Ok(HopsClient {
            hdfs_internal: Arc::new(connections),
            next_conn_idx: AtomicUsize::new(0),
        })
    }

    pub fn get_connection(&self) -> Arc<Connection> {
        let curr_index = loop {
            let current = self.next_conn_idx.load(Ordering::SeqCst);
            let new = (current + 1) % MAX_CONNECTIONS;
            match self.next_conn_idx.compare_exchange(
                current,
                new,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(prev) => break prev,
                Err(_) => continue,
            }
        };
        Arc::clone(&self.hdfs_internal[curr_index])
    }

    fn hopsfs_connect_with_url(uri: &str) -> Result<*const hdfsFS> {
        let (host_str, port_u16) = extract_host_and_port(uri);

        let c_host = CString::new(host_str).expect("CString conversion failed");
        let c_port: c_ushort = port_u16;

        let max_retries = 3;
        let mut attempt = 0;
        while attempt < max_retries {
            unsafe {
                let fs = native::hdfsConnect(c_host.as_ptr(), c_port);
                if !fs.is_null() {
                    return Ok(fs);
                }
            }
            attempt += 1;
            thread::sleep(Duration::from_millis(500));
        }

        Err(HdfsError::OperationFailed(format!(
            "Connection to HopsFS failed after {} attempts! {}",
            max_retries,
            uri
        )))
    }

    pub fn with_config(url: &str, config: HashMap<String, String>) -> Result<Self> {
        let mut connections = Vec::with_capacity(MAX_CONNECTIONS);
        for _ in 0..MAX_CONNECTIONS {
            let fs = Self::hopsfs_connect_with_config(url, &config)?;
            let connection = Arc::new(Connection::new(fs));
            connections.push(connection);
        }
        Ok(HopsClient {
            hdfs_internal: Arc::new(connections),
            next_conn_idx: AtomicUsize::new(0),
        })
    }

    fn hopsfs_connect_with_config(
        url: &str,
        config: &HashMap<String, String>,
    ) -> Result<*const hdfsFS> {
        if let (Some(username), Some(cert_dir)) = (config.get("username"), config.get("cert_dir")) {
            return Self::hopsfs_connect_as_user_with_tls(
                url,
                cert_dir,
                username,
            );
        }
        let (host_str, port_u16) = extract_host_and_port(url);
        let c_host = CString::new(host_str).expect("CString conversion failed");
        let c_port: c_ushort = port_u16;

        unsafe {
            let builder = native::hdfsNewBuilder();
            if builder.is_null() {
                return Err(HdfsError::OperationFailed(
                    "Failed to create HopsFS builder".to_string(),
                ));
            }

            native::hdfsBuilderSetNameNode(builder, c_host.as_ptr());
            native::hdfsBuilderSetNameNodePort(builder, c_port);
            native::hdfsBuilderSetForceNewInstance(builder);

            for (key, value) in config.iter() {
                let c_key = CString::new(key.as_str())
                    .map_err(|_| HdfsError::OperationFailed("Invalid config key".to_string()))?;
                let c_value = CString::new(value.as_str())
                    .map_err(|_| HdfsError::OperationFailed("Invalid config value".to_string()))?;
                native::hdfsBuilderConfSetStr(builder, c_key.as_ptr(), c_value.as_ptr());
            }

            let fs = native::hdfsBuilderConnect(builder);
            if fs.is_null() {
                return Err(HdfsError::OperationFailed(format!(
                    "Connection to HopsFS failed! {}",
                    url
                )));
            }
            Ok(fs)
        }
    }

    fn hopsfs_connect_as_user_with_tls(
        url: &str,
        cert_dir: &str,
        username: &str,
    ) -> Result<*const hdfsFS> {
        let (host_str, port_u16) = extract_host_and_port(url);
        let c_host = CString::new(host_str).expect("CString conversion failed");
        let c_cert_dir = CString::new(cert_dir)
            .expect("CString conversion of cert_dir failed");
        let c_username = CString::new(username)
            .expect("CString conversion of username failed");
        let c_port: c_ushort = port_u16;

        unsafe {
            let builder = native::hdfsNewBuilder();
            if builder.is_null() {
                return Err(HdfsError::OperationFailed(
                    "Failed to create HopsFS builder".to_string(),
                ));
            }

            native::hdfsBuilderSetNameNode(builder, c_host.as_ptr());
            native::hdfsBuilderSetNameNodePort(builder, c_port);
            native::hdfsBuilderSetForceNewInstance(builder);
            native::hdfsBuilderSetUserName(
                builder,
                c_username.as_ptr(),
            );

            let fs = native::hdfsBuilderConnectWithTLS(builder, c_cert_dir.as_ptr());
            if fs.is_null() {
                return Err(HdfsError::OperationFailed(format!(
                    "Connection to HopsFS failed! {}",
                    url
                )));
            }
            Ok(fs)
        }
    }

    pub async fn check_file_exists(&self, path: &str) -> Result<bool> {
        let connection = self.get_connection();
        let c_path = CString::new(path).unwrap();
        let res = task::spawn_blocking(move || unsafe {
            let result = native::hdfsExists(connection.get_conn_ptr(), c_path.as_ptr());
            result == 0
        })
        .await;

        if res.is_err() {
            return Err(HdfsError::OperationFailed(
                "Failed to check file existence".to_string(),
            ));
        }
        Ok(res.unwrap())
    }
    pub async fn get_file_info(&self, path: &str) -> Result<FileStatus> {

            let refined_path = CString::new(path).unwrap();
            let path_owned = path.to_string();
            let connection = self.get_connection();

            let file_status = task::spawn_blocking(move || unsafe {
                let path_info =
                    native::hdfsGetPathInfo(connection.get_conn_ptr(), refined_path.as_ptr());

                if path_info.is_null() {
                    return Err(HdfsError::FileNotFound(path_owned));
                }

                let owner = CStr::from_ptr((*path_info).mOwner)
                    .to_str()
                    .ok()
                    .map(|s| s.to_owned());

                let group = CStr::from_ptr((*path_info).mGroup)
                    .to_str()
                    .ok()
                    .map(|s| s.to_owned());

                let status = FileStatus {
                    path: path_owned,
                    length: (*path_info).mSize as usize,
                    isdir: (*path_info).mKind == tObjectKind::kObjectKindDirectory,
                    permission: (*path_info).mPermissions as u16,
                    owner: owner.unwrap(),
                    group: group.unwrap(),
                    modification_time: (*path_info).mLastMod as u64,
                    access_time: (*path_info).mLastAccess as u64,
                    replication: Some((*path_info).mReplication as u32),
                    blocksize: Some((*path_info).mBlockSize as u64),
                };

                native::hdfsFreeFileInfo(path_info, 1);

                Ok(status)
            })
            .await
            .map_err(|_| HdfsError::FileNotFound(path.to_string()))??;

            Ok(file_status)
    }

    pub async fn open_for_read(&self, path: &str) -> Result<FileReader> {
        if self.get_file_info(path).await.is_err() {
            Err(HdfsError::FileNotFound(path.to_string()))?
        }
        let c_path = CString::new(path)
            .map_err(|_| HdfsError::OperationFailed("Invalid path".to_string()))?;
        let connection = self.get_connection();
        let connection_clone = Arc::clone(&connection);

        let file_reader = task::spawn_blocking(move || unsafe {
            let hdfs_file = native::hdfsOpenFile(
                connection.get_conn_ptr(),
                c_path.as_ptr(),
                O_RDONLY,
                0,
                0,
                0,
            );

            if hdfs_file.is_null() {
                Err(HdfsError::OperationFailed(
                    "Failed to open file".to_string(),
                ))
            } else {
                Ok(FileReader::new(connection_clone, hdfs_file.cast_mut()))
            }
        })
        .await
        .map_err(|_| HdfsError::OperationFailed("Failed to open file".to_string()))??;

        Ok(file_reader)
    }

    pub async fn create(&self, path: &str, opts: WriteOptions) -> Result<FileWriter> {
        let file_exists = self.check_file_exists(path).await?;
        if file_exists && !opts.overwrite {
            return Err(HdfsError::AlreadyExists(path.to_string()));
        }

        let c_path = CString::new(path)
            .map_err(|_| HdfsError::OperationFailed("Invalid path".to_string()))?;
        let connection = self.get_connection();

        let file_writer = task::spawn_blocking(move || unsafe {
            let result = native::hdfsOpenFile(
                connection.get_conn_ptr(),
                c_path.as_ptr(),
                O_WRONLY,
                opts.buffer_size,
                opts.replication.unwrap_or(0),
                opts.block_size.unwrap_or(0) as int32_t,
            );

            if result.is_null() {
                Err(HdfsError::OperationFailed(
                    "Failed to create file".to_string(),
                ))
            } else {
                Ok(FileWriter::new(connection, result.cast_mut()))
            }
        })
        .await
        .map_err(|_| HdfsError::OperationFailed("Failed to create file".to_string()))??;

        Ok(file_writer)
    }

    pub async fn rename(&self, from: &str, to: &str, overwrite: bool) -> Result<()> {
            let destination_exists = self.check_file_exists(to).await?;
            if destination_exists && !overwrite {
                Err(HdfsError::AlreadyExists(to.to_string()))?
            }

            let _from = CString::new(from).unwrap();
            let _to = CString::new(to).unwrap();
            let connection = self.get_connection();

            let res = task::spawn_blocking(move || unsafe {
                native::hdfsRename(connection.get_conn_ptr(), _from.as_ptr(), _to.as_ptr())
            })
            .await;

            if res.is_err() || res.unwrap() != 0 {
                return Err(HdfsError::OperationFailed("rename failed!".to_string()));
            }
        Ok(())
    }

    pub async fn delete(&self, path: &str, _recursive: bool) -> Result<bool> {
        let _path = CString::new(path).unwrap();
        let connection = self.get_connection();

        let res = task::spawn_blocking(move || unsafe {
            native::hdfsDelete(
                connection.get_conn_ptr(),
                _path.as_ptr(),
                _recursive as c_int,
            )
        })
        .await;

        if res.is_err() || res.unwrap() == -1 {
            return Err(HdfsError::OperationFailed(
                "Failed to delete file".to_string(),
            ));
        }

        Ok(true)
    }

    pub async fn list_directory(&self, prefix: &str) -> Result<Vec<FileStatus>> {
        let path_cstr =
            CString::new(prefix).map_err(|_| HdfsError::InvalidPath(prefix.to_string()))?;

        let mut num_entries: c_int = 0;
        let connection = self.get_connection();
        unsafe {
            let response = native::hdfsListDirectory(
                connection.get_conn_ptr(),
                path_cstr.as_ptr(),
                &mut num_entries,
            );
            if response.is_null() || num_entries == 0 {
                return Ok(vec![]);
            }

            let file_infos = std::slice::from_raw_parts(response, num_entries as usize);
            let mut objects = Vec::with_capacity(num_entries as usize);

            for info in file_infos.iter() {
                match FileStatus::from_hdfs_file_info(info) {
                    Ok(file_info) => objects.push(file_info),
                    Err(_e) => Err(HdfsError::OperationFailed(
                        "Listing directory content failed!".to_string(),
                    ))?,
                }
            }
            native::hdfsFreeFileInfo(response, num_entries);
            Ok(objects)
        }
    }

    pub async fn hdfs_copy(&self, src: &str, dst: &str, overwrite: bool) -> Result<()> {
        let dst_exists = self.check_file_exists(dst).await?;
        if dst_exists && !overwrite {
            Err(HdfsError::AlreadyExists(dst.to_string()))?
        }

        let src_exists = self.check_file_exists(src).await?;
        if !src_exists {
            Err(HdfsError::FileNotFound(src.to_string()))?
        }

        let src_cstr = CString::new(src).unwrap();
        let dst_cstr = CString::new(dst).unwrap();
        let connection = self.get_connection();

        let res = task::spawn_blocking(move || unsafe {
            let conn_ptr_usize = connection.get_conn_ptr();
            native::hdfsCopy(
                conn_ptr_usize,
                src_cstr.as_ptr(),
                conn_ptr_usize,
                dst_cstr.as_ptr(),
            )
        })
        .await;

        if res.is_ok() || res.unwrap() == 0 {
            Ok(())
        } else {
            Err(HdfsError::OperationFailed(
                "Failed to copy file".to_string(),
            ))
        }
    }
    pub async fn mkdir(&self, path: &str) -> Result<()> {
        let path_cstr = CString::new(path).unwrap();
        let connection = self.get_connection();

        let res = task::spawn_blocking(move || unsafe {
            native::hdfsCreateDirectory(connection.get_conn_ptr(), path_cstr.as_ptr())
        })
        .await;

        if res.is_err() || res.unwrap() == -1 {
            return Err(HdfsError::OperationFailed(
                "Failed to create directory".to_string(),
            ));
        }

        Ok(())
    }
}

fn extract_host_and_port(uri: &str) -> (String, u16) {
    let default_port = 8020;

    let stripped_uri = uri
        .strip_prefix("hdfs://")
        .or_else(|| uri.strip_prefix("hopsfs://"))
        .unwrap_or(uri);

    let mut parts = stripped_uri.split(':');
    let host = parts.next().unwrap_or("").to_string();
    let port = parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(default_port);

    (host, port)
}

pub struct ReadRangeStream {
    connection: Arc<Connection>,
    file_pointer: Arc<AtomicPtr<hdfsFile>>,
    current: usize,
    end: usize,
}

impl ReadRangeStream {
    pub fn new(
        connection: Arc<Connection>,
        file_pointer: Arc<AtomicPtr<hdfsFile>>,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            connection,
            file_pointer,
            current: start,
            end,
        }
    }
}

impl Stream for ReadRangeStream {
    type Item = Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.current >= self.end {
            return Poll::Ready(None);
        }

        let current_end = std::cmp::min(self.current + DATA_BLOCK_SIZE, self.end);
        let length = current_end - self.current;
        let mut buffer = vec![0u8; length];

        unsafe {
            let result = native::hdfsPread(
                self.connection.get_conn_ptr(),
                self.file_pointer.load(Ordering::SeqCst),
                self.current as i64,
                buffer.as_mut_ptr() as *mut c_void,
                length as tSize,
            );
            if result <= 0 || (result as usize != length && current_end <= self.end) {
                return Poll::Ready(Some(Err(HdfsError::OperationFailed(format!(
                    "Failed to read {} bytes from {} with result value {}",
                    length, self.current, result
                )))));
            }
            self.current = current_end;
            let result_bytes = Bytes::copy_from_slice(&buffer[..result as usize]);
            Poll::Ready(Some(Ok(result_bytes)))
        }
    }
}

impl Drop for ReadRangeStream {
    fn drop(&mut self) {
        unsafe {
            native::hdfsCloseFile(
                self.connection.get_conn_ptr(),
                self.file_pointer.load(Ordering::SeqCst),
            );
        }
    }
}
