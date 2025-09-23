use hdfs_native_object_store::{HopsClient, WriteOptions, HdfsError};
use tokio::fs;

type Result<T> = std::result::Result<T, HdfsError>;

/// Demonstrates the enhanced copy operations API with overwrite and parent directory creation.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let client = HopsClient::with_url("hopsfs://localhost:8020")?;
    
    println!("=== Enhanced Copy Operations Demo ===\n");
    
    // Setup test file
    fs::write("/tmp/source.txt", "Hello, HDFS copy operations!").await
        .map_err(|_| HdfsError::OperationFailed("Failed to create test file".to_string()))?;
    
    // 1. HDFS operations with WriteOptions
    println!("1. HDFS Copy Operations:");
    
    // Safe copy (no overwrite)
    client.copy_from_local("/tmp/source.txt", "/hdfs/safe_file.txt", None).await?;
    println!("   ✓ Safe copy to HDFS (no overwrite)");
    
    // Custom settings with overwrite
    let custom_opts = WriteOptions {
        overwrite: true,
        block_size: Some(64 * 1024 * 1024), // 64MB blocks
        ..Default::default()
    };
    client.copy_from_local("/tmp/source.txt", "/hdfs/custom_file.txt", Some(custom_opts)).await?;
    println!("   ✓ Custom HDFS copy with overwrite");
    
    // 2. Local operations with boolean flags
    println!("\n2. Local Copy Operations:");
    
    // Basic copy (no overwrite, no parent creation)
    client.copy_to_local("/hdfs/safe_file.txt", "/tmp/copied_basic.txt", false, false).await?;
    println!("   ✓ Basic copy to local");
    
    // With overwrite
    client.copy_to_local("/hdfs/safe_file.txt", "/tmp/copied_basic.txt", true, false).await?;
    println!("   ✓ Copy with overwrite");
    
    // With parent directory creation
    client.copy_to_local("/hdfs/safe_file.txt", "/tmp/deep/nested/path/file.txt", false, true).await?;
    println!("   ✓ Copy with parent directory creation");
    
    // With both overwrite and parent creation
    client.copy_to_local("/hdfs/safe_file.txt", "/tmp/another/deep/path/file.txt", true, true).await?;
    println!("   ✓ Copy with overwrite and parent creation");
    
    // 3. File system operations
    println!("\n3. File System Operations:");
    
    // Change permissions (644 = rw-r--r--)
    client.chmod("/hdfs/safe_file.txt", 0o644).await?;
    println!("   ✓ Changed file permissions to 644");
    
    // Change ownership (pass None to keep current owner/group)
    client.chown("/hdfs/safe_file.txt", Some("newuser"), None).await
        .unwrap_or_else(|_| println!("   ⚠ chown failed (may need admin privileges)"));
    println!("   ✓ Attempted ownership change");
    
    println!("\n=== API Design Benefits ===");
    println!("✓ HDFS operations: WriteOptions for complex settings");
    println!("✓ Local operations: Simple boolean flags");  
    println!("✓ Parent directory creation: Automatic when requested");
    println!("✓ File system operations: Direct chmod/chown support");
    println!("✓ Clear separation: HDFS vs local filesystem concerns");
    println!("✓ Safe defaults: No overwrites or directory creation unless explicit");
    
    // Cleanup
    let _ = fs::remove_file("/tmp/source.txt").await;
    let _ = fs::remove_file("/tmp/copied_basic.txt").await;
    let _ = fs::remove_dir_all("/tmp/deep").await;
    let _ = fs::remove_dir_all("/tmp/another").await;
    
    Ok(())
}