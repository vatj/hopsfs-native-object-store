# HDFS Copy Operations - API Design Improvement

## Summary of Changes

The copy methods have been refactored to use `WriteOptions` for overwrite control, making the API more cohesive and intuitive. Additionally, the default values for `WriteOptions` have been updated to be more suitable for modern filesystems.

## Updated WriteOptions Defaults

The `WriteOptions::default()` now provides modern, optimized defaults:

```rust
WriteOptions {
    block_size: Some(128 * 1024 * 1024), // 128MB - optimal for modern HDFS clusters
    replication: None,                    // Use cluster default (typically 3)
    overwrite: false,                     // Safe by default
    create_parent: true,                  // Convenient default behavior  
    buffer_size: 64 * 1024,              // 64KB - good balance of throughput and memory
}
```

### Rationale for New Defaults

**Block Size: 128MB (was: None)**
- Modern HDFS clusters perform better with larger block sizes
- 128MB is a sweet spot for most workloads (large enough for efficiency, not too large for memory)
- Better for large files while still reasonable for smaller files
- Aligns with modern Hadoop best practices

**Buffer Size: 64KB (was: 0)**  
- 0 caused HDFS to use very small system defaults (often 4KB or 8KB)
- 64KB provides much better I/O throughput for network and disk operations
- Good balance between memory usage and performance
- Typical for modern distributed storage systems

## API Comparison

### Before (Separate overwrite parameter)
```rust
// Old API - overwrite as separate boolean parameter
client.copy_from_local("/local/file.txt", "/hdfs/file.txt", write_opts, true).await?;
client.copy_to_local("/hdfs/file.txt", "/local/file.txt", true).await?;
```

### After (WriteOptions-based overwrite control)
```rust
// New API - overwrite controlled via WriteOptions
client.copy_from_local("/local/file.txt", "/hdfs/file.txt", None).await?;  // Safe default
client.copy_to_local("/hdfs/file.txt", "/local/file.txt", None).await?;    // Safe default

// With custom WriteOptions including overwrite
let opts = WriteOptions { overwrite: true, ..Default::default() };
client.copy_from_local("/local/file.txt", "/hdfs/file.txt", Some(opts)).await?;
client.copy_to_local("/hdfs/file.txt", "/local/file.txt", Some(opts)).await?;
```

## Method Signatures

### `copy_from_local`
```rust
pub async fn copy_from_local(
    &self, 
    local_path: &str, 
    hdfs_path: &str, 
    opts: Option<WriteOptions>  // ← Optional WriteOptions controls everything
) -> Result<()>
```

### `copy_to_local` 
```rust
pub async fn copy_to_local(
    &self, 
    hdfs_path: &str, 
    local_path: &str, 
    opts: Option<WriteOptions>  // ← Consistent with copy_from_local
) -> Result<()>
```

## Usage Patterns

### 1. Safe Defaults (Recommended)
```rust
// No WriteOptions = safe defaults (no overwrite)
client.copy_from_local("/tmp/file.txt", "/hdfs/file.txt", None).await?;
client.copy_to_local("/hdfs/file.txt", "/tmp/copy.txt", None).await?;

// Convenience methods for common safe operations
client.copy_from_local_safe("/tmp/file.txt", "/hdfs/file.txt").await?;
client.copy_to_local_safe("/hdfs/file.txt", "/tmp/copy.txt").await?;
```

### 2. Custom Settings without Overwrite
```rust
let safe_opts = WriteOptions {
    overwrite: false,                    // Explicit no overwrite
    block_size: Some(128 * 1024 * 1024), // 128MB blocks
    replication: Some(3),                // 3x replication
    create_parent: true,
    buffer_size: 16384,
};

client.copy_from_local("/tmp/file.txt", "/hdfs/file.txt", Some(safe_opts)).await?;
```

### 3. Allow Overwrite with Custom Settings
```rust
let overwrite_opts = WriteOptions {
    overwrite: true,                     // Allow overwrite
    block_size: Some(64 * 1024 * 1024),  // 64MB blocks
    replication: Some(2),                // 2x replication
    create_parent: true,
    buffer_size: 8192,
};

client.copy_from_local("/tmp/file.txt", "/hdfs/file.txt", Some(overwrite_opts)).await?;
client.copy_to_local("/hdfs/file.txt", "/tmp/copy.txt", Some(overwrite_opts)).await?;
```

## Benefits of the New Design

✅ **Cohesive API**: All write-related settings (overwrite, replication, block size) in one place  
✅ **Safe by Default**: `None` = safe defaults with no overwrite risk  
✅ **Consistent**: Both copy methods use the same pattern  
✅ **Flexible**: Supports all combinations of settings  
✅ **Clear Intent**: Overwrite requires explicit WriteOptions configuration  
✅ **Future-Proof**: Easy to add new write options without changing method signatures  
✅ **Backward Compatible**: Existing WriteOptions usage still works  

## Error Handling

The API provides clear error messages:
- `HdfsError::AlreadyExists` when destination exists and `overwrite: false`
- `HdfsError::FileNotFound` when source file doesn't exist
- `HdfsError::InvalidPath` when source is not a regular file
- `HdfsError::OperationFailed` for I/O or permission errors

## Migration Guide

If you have existing code using the old API pattern, simply wrap your WriteOptions in `Some()`:

```rust
// Old
client.copy_from_local(local, hdfs, opts, true).await?;

// New  
let mut opts = opts;
opts.overwrite = true;
client.copy_from_local(local, hdfs, Some(opts)).await?;
```

For safe operations, just pass `None`:
```rust
client.copy_from_local(local, hdfs, None).await?;  // Safe default
```