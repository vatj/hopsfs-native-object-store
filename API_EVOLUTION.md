# API Evolution and Design Decisions

## Key Evolution Points

### 1. FileReader Enhancement
Enhanced from basic file reader to full async reading support with `hdfsRead` integration, including `read_to_string` convenience method.

### 2. Copy Operations Implementation
Added `copy_from_local` and `copy_to_local` with chunked async I/O and proper error handling.

### 3. File System Operations
Added `chmod` and `chown` passthrough methods for HDFS file permission and ownership management.

### 4. Overwrite and Parent Directory Logic Evolution
**Phase 1**: Simple boolean flags
```rust
copy_from_local(local_path, hdfs_path, overwrite: bool)
copy_to_local(hdfs_path, local_path, overwrite: bool)
```

**Phase 2**: WriteOptions integration  
```rust
copy_from_local(local_path, hdfs_path, write_options: Option<WriteOptions>)
copy_to_local(hdfs_path, local_path, write_options: Option<WriteOptions>)
```

**Phase 3 (Current)**: Context-appropriate handling + Parent directory creation
```rust
copy_from_local(local_path, hdfs_path, write_options: Option<WriteOptions>)        // HDFS-specific
copy_to_local(hdfs_path, local_path, overwrite: bool, create_parent: bool)         // Local filesystem-specific
```

## Design Rationale

### HDFS Operations (copy_from_local)
Uses `WriteOptions` because HDFS has complex write semantics (block size, replication, etc.). Optional WriteOptions with safe defaults (no overwrite).

### Local Operations (copy_to_local)
Uses boolean flags because local filesystem is simpler:
- `overwrite`: Controls file replacement behavior  
- `create_parent`: Controls automatic parent directory creation via `tokio::fs::create_dir_all`
- Leverages `tokio::fs::OpenOptions` for atomic file creation

## Modern Defaults
- **Block size**: 128MB (optimal for modern systems)
- **Buffer size**: 64KB (balanced memory vs I/O efficiency)  
- **Overwrite**: false (safe by default)
- **Create parent**: false (explicit opt-in required)

## Usage Patterns

### Safe Operations
```rust
// HDFS copy (no overwrite)
client.copy_from_local("local.txt", "/hdfs/remote.txt", None).await?;

// Local copy (no overwrite, no parent creation)
client.copy_to_local("/hdfs/remote.txt", "local_copy.txt", false, false).await?;
```

### With Parent Directory Creation
```rust
// Local copy with automatic parent directory creation
client.copy_to_local("/hdfs/remote.txt", "deep/path/local_copy.txt", false, true).await?;

// File system operations
client.chmod("/hdfs/file.txt", 0o755).await?;                                  // Change permissions
client.chown("/hdfs/file.txt", Some("user"), Some("group")).await?;            // Change ownership
```

### Explicit Control
```rust
// HDFS copy with custom settings
let opts = WriteOptions { overwrite: true, block_size: 256*1024*1024, ..Default::default() };
client.copy_from_local("local.txt", "/hdfs/remote.txt", Some(opts)).await?;

// Local copy with overwrite and parent creation
client.copy_to_local("/hdfs/remote.txt", "new/path/local_copy.txt", true, true).await?;
```