use hdfs_native_object_store::client::{HopsClient, WriteOptions, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Create HopsFS client
    let client = HopsClient::with_url("hopsfs://localhost:8020")?;
    
    println!("=== HDFS Copy Operations with Improved WriteOptions API ===\n");
    
    // Example 1: Safe copy operations (default behavior, no overwrite)
    println!("1. Safe copy operations (no WriteOptions = no overwrite)...");
    
    client.copy_from_local("/tmp/local_file.txt", "/hdfs/remote_file.txt", None).await?;
    println!("   ✓ File successfully copied to HDFS with safe defaults!");
    
    // Example 2: Try to copy again - should fail since no overwrite specified
    println!("2. Attempting to copy again without overwrite...");
    match client.copy_from_local("/tmp/local_file.txt", "/hdfs/remote_file.txt", None).await {
        Ok(_) => println!("   ⚠ Copy succeeded (file didn't exist)"),
        Err(e) => println!("   ✓ Copy failed as expected: {}", e),
    }
    
    // Example 3: Copy with explicit overwrite using WriteOptions
    println!("3. Copy with explicit overwrite in WriteOptions...");
    let overwrite_opts = WriteOptions {
        overwrite: true,
        block_size: Some(64 * 1024 * 1024), // 64MB blocks
        replication: Some(3),                // 3x replication  
        create_parent: true,
        buffer_size: 8192,
    };
    
    client.copy_from_local(
        "/tmp/local_file.txt",
        "/hdfs/remote_file.txt", 
        Some(overwrite_opts)
    ).await?;
    println!("   ✓ File successfully overwritten on HDFS!");
    
    // Example 4: Copy from HDFS to local (safe by default)
    println!("4. Copy from HDFS to local (safe by default)...");
    client.copy_to_local("/hdfs/remote_file.txt", "/tmp/copied_file.txt", None).await?;
    println!("   ✓ File successfully copied from HDFS!");
    
    // Example 5: Copy to local with overwrite
    println!("5. Copy to local with overwrite...");
    let local_overwrite_opts = WriteOptions {
        overwrite: true,
        ..Default::default()
    };
    
    client.copy_to_local(
        "/hdfs/remote_file.txt", 
        "/tmp/copied_file.txt",
        Some(local_overwrite_opts)
    ).await?;
    println!("   ✓ File successfully overwritten locally!");
    
    // Example 6: Using convenience methods
    println!("6. Using convenience methods for safe operations...");
    client.copy_from_local_safe("/tmp/another_file.txt", "/hdfs/safe_file.txt").await?;
    client.copy_to_local_safe("/hdfs/safe_file.txt", "/tmp/safe_copy.txt").await?;
    println!("   ✓ Safe convenience methods work perfectly!");
    
    println!("\n=== Benefits of the New API ===");
    println!("✓ WriteOptions controls overwrite behavior cohesively");
    println!("✓ Optional WriteOptions = safe defaults (no overwrite)"); 
    println!("✓ Explicit overwrite requires intentional WriteOptions");
    println!("✓ All HDFS write settings in one place");
    println!("✓ Clean, consistent API design");
    println!("✓ Backward compatible with existing WriteOptions usage");
    
    Ok(())
}