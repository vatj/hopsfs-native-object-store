use hdfs_native_object_store::client::{HopsClient, WriteOptions, Result};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Create HopsFS client
    let client = HopsClient::with_url("hopsfs://localhost:8020")?;
    
    println!("=== HDFS Copy Operations with WriteOptions-Based Overwrite Control ===\n");
    
    // Test 1: Safe copy operations (None = no overwrite)
    println!("1. Testing safe copy operations (None WriteOptions)...");
    
    // Copy from local to HDFS safely
    match client.copy_from_local("/tmp/source.txt", "/hdfs/target.txt", None).await {
        Ok(_) => println!("   ✓ Safe copy to HDFS succeeded"),
        Err(e) => println!("   ✗ Safe copy to HDFS failed: {}", e),
    }
    
    // Try to copy again - should fail if file exists
    match client.copy_from_local("/tmp/source.txt", "/hdfs/target.txt", None).await {
        Ok(_) => println!("   ⚠ Safe copy succeeded (file didn't exist)"),
        Err(e) => println!("   ✓ Safe copy correctly failed: {}", e),
    }
    
    println!();
    
    // Test 2: Copy with WriteOptions controlling overwrite
    println!("2. Testing WriteOptions-based overwrite control...");
    
    let write_opts = WriteOptions {
        overwrite: true,                     // Explicit overwrite permission
        block_size: Some(64 * 1024 * 1024), // 64MB blocks
        replication: Some(3),                // 3x replication
        create_parent: true,
        buffer_size: 8192,
    };
    
    // Copy with overwrite=true in WriteOptions
    match client.copy_from_local("/tmp/source.txt", "/hdfs/target.txt", Some(write_opts)).await {
        Ok(_) => println!("   ✓ Overwrite copy to HDFS succeeded"),
        Err(e) => println!("   ✗ Overwrite copy to HDFS failed: {}", e),
    }
    
    println!();
    
    // Test 3: Copy from HDFS to local with WriteOptions control  
    println!("3. Testing HDFS to local copy with WriteOptions...");
    
    // Safe copy to local (None = no overwrite)
    match client.copy_to_local("/hdfs/target.txt", "/tmp/copied_safe.txt", None).await {
        Ok(_) => println!("   ✓ Safe copy to local succeeded"),
        Err(e) => println!("   ✗ Safe copy to local failed: {}", e),
    }
    
    // Copy with overwrite using WriteOptions
    let local_overwrite_opts = WriteOptions {
        overwrite: true,
        ..Default::default()
    };
    
    match client.copy_to_local(
        "/hdfs/target.txt", 
        "/tmp/copied_overwrite.txt", 
        Some(local_overwrite_opts)
    ).await {
        Ok(_) => println!("   ✓ Overwrite copy to local succeeded"),
        Err(e) => println!("   ✗ Overwrite copy to local failed: {}", e),
    }
    
    // Try to copy without overwrite - should fail if file exists
    match client.copy_to_local("/hdfs/target.txt", "/tmp/copied_overwrite.txt", None).await {
        Ok(_) => println!("   ⚠ No-overwrite copy succeeded (file didn't exist)"),
        Err(e) => println!("   ✓ No-overwrite copy correctly failed: {}", e),
    }
    
    println!();
    
    // Test 4: Using convenience methods
    println!("4. Testing convenience methods...");
    
    match client.copy_from_local_safe("/tmp/source.txt", "/hdfs/safe_target.txt").await {
        Ok(_) => println!("   ✓ copy_from_local_safe succeeded"),
        Err(e) => println!("   ✗ copy_from_local_safe failed: {}", e),
    }
    
    match client.copy_to_local_safe("/hdfs/safe_target.txt", "/tmp/safe_copy.txt").await {
        Ok(_) => println!("   ✓ copy_to_local_safe succeeded"), 
        Err(e) => println!("   ✗ copy_to_local_safe failed: {}", e),
    }
    
    println!();
    
    // Test 5: Demonstrate different WriteOptions configurations
    println!("5. Testing different WriteOptions configurations...");
    println!("   The new API provides:");
    println!("   • None = Safe defaults (no overwrite, default replication/block size)");
    println!("   • Some(WriteOptions{overwrite: false, ...}) = Custom settings, no overwrite");
    println!("   • Some(WriteOptions{overwrite: true, ...}) = Custom settings, allow overwrite");
    println!("   • Convenience methods for common safe operations");
    
    println!("\n=== WriteOptions-Based Copy Operations Complete ===");
    
    Ok(())
}

// Example demonstrating different WriteOptions patterns
async fn demonstrate_write_options_patterns() -> Result<()> {
    let client = HopsClient::with_url("hopsfs://localhost:8020")?;
    
    // Pattern 1: Safe defaults (no overwrite)
    client.copy_from_local("/tmp/file.txt", "/hdfs/file.txt", None).await?;
    
    // Pattern 2: Custom settings with no overwrite
    let safe_custom_opts = WriteOptions {
        overwrite: false,
        block_size: Some(128 * 1024 * 1024), // 128MB blocks
        replication: Some(2),                 // 2x replication
        create_parent: true,
        buffer_size: 16384,
    };
    client.copy_from_local("/tmp/file.txt", "/hdfs/custom.txt", Some(safe_custom_opts)).await?;
    
    // Pattern 3: Allow overwrite with custom settings
    let overwrite_opts = WriteOptions {
        overwrite: true,
        block_size: Some(256 * 1024 * 1024), // 256MB blocks
        replication: Some(1),                 // Single copy
        create_parent: true,
        buffer_size: 32768,
    };
    client.copy_from_local("/tmp/file.txt", "/hdfs/overwrite.txt", Some(overwrite_opts)).await?;
    
    // Pattern 4: Convenience methods for simple operations
    client.copy_from_local_safe("/tmp/file.txt", "/hdfs/simple.txt").await?;
    client.copy_to_local_safe("/hdfs/simple.txt", "/tmp/simple_copy.txt").await?;
    
    Ok(())
}