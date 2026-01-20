mod grib;
mod s3;

use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate};
use clap::Parser;
use futures::StreamExt;

use crate::grib::{is_wind_message, Grib2StreamParser};
use crate::s3::S3MultipartUploader;

#[derive(Parser, Debug)]
#[command(author, version, about = "Download GFS wind data and stream to S3")]
struct Args {
    /// Start date (YYYY-MM-DD)
    #[arg(short, long)]
    start_date: String,

    /// End date (YYYY-MM-DD)
    #[arg(short, long)]
    end_date: String,

    /// S3 bucket name
    #[arg(short, long)]
    bucket: String,

    /// S3 key prefix (e.g., "wind/2020/")
    #[arg(short, long, default_value = "")]
    prefix: String,

    /// AWS region (defaults to AWS_REGION env var or us-east-1)
    #[arg(long)]
    region: Option<String>,
}

/// Process a single GFS file: download, filter wind messages, upload to S3.
async fn process_file(
    http: &reqwest::Client,
    s3: &aws_sdk_s3::Client,
    date: NaiveDate,
    hour: &str,
    bucket: &str,
    prefix: &str,
) -> Result<()> {
    let date_str = date.format("%Y%m%d").to_string();
    let year = date.format("%Y").to_string();

    // NCAR RDA URL structure
    let url = format!(
        "https://data.rda.ucar.edu/ds084.1/{year}/{date_str}/gfs.0p25.{date_str}{hour}.f000.grib2"
    );

    // S3 key
    let key = if prefix.is_empty() {
        format!("wind_{date_str}_{hour}.grb2")
    } else {
        let p = prefix.trim_end_matches('/');
        format!("{p}/wind_{date_str}_{hour}.grb2")
    };

    println!("Processing: {date} {hour} -> s3://{bucket}/{key}");

    // Start HTTP download stream
    let response = http
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to request {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} for {}", response.status(), url);
    }

    let total_size = response.content_length();
    let mut stream = response.bytes_stream();

    // Start S3 multipart upload
    let mut uploader = S3MultipartUploader::new(s3.clone(), bucket, &key).await?;
    let mut parser = Grib2StreamParser::new();

    let mut downloaded: u64 = 0;
    let mut wind_messages: u64 = 0;
    let mut total_messages: u64 = 0;

    // Process stream
    loop {
        match stream.next().await {
            Some(Ok(chunk)) => {
                downloaded += chunk.len() as u64;

                // Parse GRIB2 messages from chunk
                for msg in parser.feed(&chunk) {
                    total_messages += 1;

                    if is_wind_message(&msg) {
                        wind_messages += 1;
                        if let Err(e) = uploader.write(&msg).await {
                            // Abort upload on error
                            let _ = uploader.abort().await;
                            return Err(e);
                        }
                    }
                }

                // Progress indicator
                if let Some(total) = total_size {
                    let pct = (downloaded as f64 / total as f64) * 100.0;
                    print!(
                        "\r  Downloaded: {pct:.1}% | Messages: {total_messages} total, {wind_messages} wind"
                    );
                } else {
                    print!(
                        "\r  Downloaded: {downloaded} bytes | Messages: {total_messages} total, {wind_messages} wind"
                    );
                }
            }
            Some(Err(e)) => {
                let _ = uploader.abort().await;
                return Err(e).context("Stream error");
            }
            None => break,
        }
    }

    println!();

    // Complete upload
    uploader.complete().await?;

    println!(
        "  Completed: {wind_messages} wind messages extracted from {total_messages} total"
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Parse dates
    let start_date = NaiveDate::parse_from_str(&args.start_date, "%Y-%m-%d")
        .context("Invalid start date format (use YYYY-MM-DD)")?;
    let end_date = NaiveDate::parse_from_str(&args.end_date, "%Y-%m-%d")
        .context("Invalid end date format (use YYYY-MM-DD)")?;

    if start_date > end_date {
        anyhow::bail!("Start date must be before or equal to end date");
    }

    println!("GFS Wind Data Downloader -> S3");
    println!("==============================");
    println!("Date range: {start_date} to {end_date}");
    println!("S3 bucket: {}", args.bucket);
    println!("S3 prefix: {}", args.prefix);
    println!();

    // Initialize AWS SDK
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&aws_config);

    // Initialize HTTP client
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    // Process each date
    let hours = ["00", "06", "12", "18"];
    let mut current_date = start_date;

    while current_date <= end_date {
        println!("=== {current_date} ===");

        for hour in &hours {
            match process_file(
                &http_client,
                &s3_client,
                current_date,
                hour,
                &args.bucket,
                &args.prefix,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("  Error processing {current_date} {hour}: {e}");
                }
            }
        }

        current_date += Duration::days(1);
    }

    println!();
    println!("Done!");

    Ok(())
}
