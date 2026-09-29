use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::Client;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use std::env;

const REGION: &str = "us-ord-10";
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load variables from .env file
    dotenvy::dotenv().ok(); // This loads the .env file into the environment

    // Fetch variables from environment
    let access_key = env::var("AKAMAI_ACCESS_KEY").expect("AKAMAI_ACCESS_KEY not set in .env");
    let secret_key = env::var("AKAMAI_SECRET_KEY").expect("AKAMAI_SECRET_KEY not set in .env");
    let endpoint = env::var("AKAMAI_ENDPOINT").expect("AKAMAI_ENDPOINT not set in .env");
    let bucket_name = env::var("AKAMAI_BUCKET_NAME").expect("AKAMAI_BUCKET_NAME not set in .env");
    let input_file = "files.txt";

    // 1. Setup AWS Config
    let credentials = Credentials::new(access_key, secret_key, None, None, "akamai");
    let config = aws_config::from_env()
        .region(RegionProviderChain::default_provider().or_else(REGION))
        .credentials_provider(credentials)
        .endpoint_url(endpoint)
        .load()
        .await;

    let client = Client::new(&config);

    // 2. Read the list of files
    let content = fs::read_to_string(input_file).await
        .expect("Could not read files.txt. Make sure it exists in the project root.");

    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

    println!("Found {} files to process.\n", lines.len());

    // 3. Process each file
    for key in lines {
        println!("Processing: {}", key);
        match process_transfer(&client, &bucket_name, key).await {
            Ok(_) => println!("Successfully processed and deleted: {}\n", key),
            Err(e) => eprintln!("Error processing {}: {}\n", key, e),
        }
    }

    Ok(())
}

/// Downloads a file from the bucket, recreates the directory structure locally,
/// and deletes it from the source.
async fn process_transfer(client: &Client, bucket_name: &str, object_key: &str) -> anyhow::Result<()> {
    // Determine the local path (e.g., "folder/subfolder/file.png")
    let local_path = std::path::Path::new(object_key);

    // Create the parent directories recursively
    if let Some(parent) = local_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).await?;
            println!("  -> Created directory structure: {:?}", parent);
        }
    }

    // Download the object
    println!("  -> Downloading {}...", object_key);
    let resp = client
        .get_object()
        .bucket(bucket_name)
        .key(object_key)
        .send()
        .await?;

    let data = resp.body.collect().await?;
    let bytes = data.to_vec();

    // Write the file
    let mut file = File::create(local_path).await?;
    file.write_all(&bytes).await?;
    
    println!("  -> Saved to local path: {:?}", local_path);

    // Delete from source
    client
        .delete_object()
        .bucket(bucket_name)
        .key(object_key)
        .send()
        .await?;

    println!("  -> Deleted from source.");
    Ok(())
}
