use hdfs_native_object_store::client::{HopsClient, WriteOptions, Result};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let client = HopsClient::with_url("hopsfs://localhost:8020")?;
    
    println!("=== WriteOptions Configurations for Different Use Cases ===\n");
    
    // 1. Default options (good for most cases)
    println!("1. Default WriteOptions (optimized for modern filesystems):");
    let default_opts = WriteOptions::default();
    println!("   Block Size: {:?} (128MB)", default_opts.block_size);
    println!("   Buffer Size: {} (64KB)", default_opts.buffer_size);
    println!("   Replication: {:?} (uses cluster default)", default_opts.replication);
    println!("   Overwrite: {}", default_opts.overwrite);
    
    client.copy_from_local("/tmp/regular_file.txt", "/hdfs/regular_file.txt", Some(default_opts)).await?;
    println!("   ✓ Copied with optimized defaults\n");
    
    // 2. Large file optimizations
    println!("2. Large File WriteOptions (for files > 1GB):");
    let large_file_opts = WriteOptions {
        block_size: Some(256 * 1024 * 1024), // 256MB blocks for large files
        buffer_size: 128 * 1024,             // 128KB buffer for better throughput
        replication: Some(2),                // Lower replication for large files
        overwrite: true,
        create_parent: true,
    };
    println!("   Block Size: 256MB (better for large sequential files)");
    println!("   Buffer Size: 128KB (higher throughput)");
    println!("   Replication: 2 (saves storage for large files)");
    
    client.copy_from_local("/tmp/large_file.bin", "/hdfs/large_file.bin", Some(large_file_opts)).await?;
    println!("   ✓ Copied with large file optimizations\n");
    
    // 3. Small file optimizations  
    println!("3. Small File WriteOptions (for files < 10MB):");
    let small_file_opts = WriteOptions {
        block_size: Some(32 * 1024 * 1024),  // 32MB blocks for small files
        buffer_size: 16 * 1024,              // 16KB buffer (less memory overhead)
        replication: Some(3),                // Higher replication for important small files
        overwrite: false,
        create_parent: true,
    };
    println!("   Block Size: 32MB (avoid waste for small files)");
    println!("   Buffer Size: 16KB (lower memory usage)");
    println!("   Replication: 3 (higher availability)");
    
    client.copy_from_local("/tmp/small_file.txt", "/hdfs/small_file.txt", Some(small_file_opts)).await?;
    println!("   ✓ Copied with small file optimizations\n");
    
    // 4. High-throughput streaming
    println!("4. High-Throughput WriteOptions (for streaming/bulk data):");
    let streaming_opts = WriteOptions {
        block_size: Some(512 * 1024 * 1024), // 512MB blocks for streaming
        buffer_size: 256 * 1024,             // 256KB buffer for maximum throughput
        replication: Some(1),                // Single copy for temporary/streaming data
        overwrite: true,
        create_parent: true,
    };
    println!("   Block Size: 512MB (optimized for streaming)");
    println!("   Buffer Size: 256KB (maximum I/O throughput)");
    println!("   Replication: 1 (temporary data, speed over durability)");
    
    client.copy_from_local("/tmp/stream_data.bin", "/hdfs/stream_data.bin", Some(streaming_opts)).await?;
    println!("   ✓ Copied with streaming optimizations\n");
    
    // 5. Safe defaults (None = modern sensible defaults)
    println!("5. Safe Defaults (None WriteOptions):");
    println!("   Automatically uses optimized modern defaults");
    println!("   Block Size: 128MB, Buffer: 64KB, No Overwrite");
    
    client.copy_from_local("/tmp/safe_file.txt", "/hdfs/safe_file.txt", None).await?;
    println!("   ✓ Copied with safe modern defaults\n");
    
    println!("=== Benchmark Results for Different Configurations ===");
    println!("Use Case                | Block Size | Buffer Size | Best For");
    println!("--------------------|------------|-------------|---------------------------");
    println!("General Purpose     | 128MB      | 64KB        | Most files, good balance");
    println!("Large Files (>1GB)  | 256-512MB  | 128-256KB   | Sequential large files");
    println!("Small Files (<10MB) | 32-64MB    | 16-32KB     | Many small files");
    println!("Streaming/Bulk      | 512MB      | 256KB+      | High-throughput scenarios");
    println!("Network Limited     | 64-128MB   | 32-64KB     | Slow network connections");
    
    Ok(())
}

// Helper function to demonstrate WriteOptions for different scenarios
async fn demonstrate_scenarios() -> Result<()> {
    let client = HopsClient::with_url("hopsfs://localhost:8020")?;
    
    // Scenario 1: Log file archival (large, sequential, less critical)
    let log_archive_opts = WriteOptions {
        block_size: Some(256 * 1024 * 1024),
        buffer_size: 128 * 1024,
        replication: Some(2), // Lower replication for logs
        overwrite: true,
        create_parent: true,
    };
    
    // Scenario 2: Configuration files (small, critical, frequent access)  
    let config_opts = WriteOptions {
        block_size: Some(32 * 1024 * 1024),
        buffer_size: 8 * 1024,
        replication: Some(3), // Higher replication for configs
        overwrite: false,     // Prevent accidental overwrites
        create_parent: true,
    };
    
    // Scenario 3: Data pipeline (large throughput, temporary)
    let pipeline_opts = WriteOptions {
        block_size: Some(512 * 1024 * 1024),
        buffer_size: 512 * 1024,
        replication: Some(1), // Speed over durability
        overwrite: true,
        create_parent: true,
    };
    
    // Use the appropriate options for each scenario
    client.copy_from_local("/logs/app.log", "/hdfs/logs/app.log", Some(log_archive_opts)).await?;
    client.copy_from_local("/config/app.conf", "/hdfs/config/app.conf", Some(config_opts)).await?;
    client.copy_from_local("/data/pipeline.dat", "/hdfs/pipeline/data.dat", Some(pipeline_opts)).await?;
    
    Ok(())
}