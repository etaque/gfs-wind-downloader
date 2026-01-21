# Architecture

This document describes the internal architecture of the GFS Wind Data Downloader.

## Project Structure

```
gfs-wind-downloader/
├── src/
│   ├── main.rs          # Entry point, CLI, orchestration
│   ├── grib.rs          # GRIB2 streaming parser and wind filtering
│   └── s3.rs            # S3 multipart upload management
├── Cargo.toml           # Rust dependencies
├── process_wind_data.py # Python utility for post-processing
├── pyproject.toml       # Python dependencies
└── flake.nix            # Nix development environment
```

## Data Flow

```
NCAR RDA Server (ds084.1)
│
│  HTTP Stream (~500 MB per file)
v
┌──────────────────────────┐
│  Grib2StreamParser       │
│  • Accumulates chunks    │
│  • Finds "GRIB" magic    │
│  • Extracts messages     │
└──────────────────────────┘
│
│  ~8,400 GRIB2 messages
v
┌──────────────────────────┐
│  Wind Filter             │
│  • Category == 2         │
│  • Number == 2 (UGRD)    │
│  • Number == 3 (VGRD)    │
└──────────────────────────┘
│
│  ~212 wind messages (~95% reduction)
v
┌──────────────────────────┐
│  S3MultipartUploader     │
│  • 5 MB buffer threshold │
│  • Upload parts on flush │
└──────────────────────────┘
│
v
S3: wind_YYYYMMDD_HH.grb2
```

## Components

### main.rs - Orchestrator

Responsibilities:
- Parse CLI arguments (clap)
- Iterate through date range with 6-hourly steps (00, 06, 12, 18 UTC)
- Initialize AWS SDK and HTTP client
- Call `process_file()` for each GFS file
- Handle errors per-file without stopping batch

URL pattern (NCAR THREDDS):
```
https://thredds.rda.ucar.edu/thredds/fileServer/files/g/d084001/{year}/{date}/gfs.0p25.{date}{hour}.f000.grib2
```

### grib.rs - GRIB2 Stream Parser

**`Grib2StreamParser`** - Stateful message extractor:
- Accumulates HTTP chunks in a `BytesMut` buffer
- `feed()`: Accepts chunks, returns complete messages
- `try_extract_message()`: Scans for "GRIB" magic bytes, reads 8-byte length field, validates "7777" terminator

**`is_wind_message()`** - Wind variable filter:
- Parses message using `grib` crate
- Checks Product Definition Section:
  - Parameter category == 2 (Momentum)
  - Parameter number == 2 (UGRD) or 3 (VGRD)
- Returns true only for wind variables

### s3.rs - S3 Multipart Uploader

**`S3MultipartUploader`** - Manages upload lifecycle:
- `new()`: Calls CreateMultipartUpload, gets upload_id
- `write()`: Appends to buffer, auto-flushes at 5 MB
- `flush_part()`: Uploads part, records ETag
- `complete()`: Finalizes with CompleteMultipartUpload
- `abort()`: Cancels upload on error

Buffer capacity: 10 MB (2x minimum part size)

## Dependencies

### Rust

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `reqwest` | HTTP client with streaming |
| `aws-sdk-s3` | S3 API |
| `grib` | GRIB2 parsing |
| `clap` | CLI parsing |
| `chrono` | Date handling |
| `anyhow` | Error handling |
| `bytes` | Buffer operations |

Uses `rustls-tls` for TLS (pure Rust, no OpenSSL).

### Python (optional)

| Package | Purpose |
|---------|---------|
| `pygrib` | GRIB2 reading |
| `numpy` | Numerical operations |
| `matplotlib` | Visualization |

## AWS Permissions Required

```json
{
  "Effect": "Allow",
  "Action": [
    "s3:PutObject",
    "s3:CreateMultipartUpload",
    "s3:UploadPart",
    "s3:CompleteMultipartUpload",
    "s3:AbortMultipartUpload"
  ],
  "Resource": "arn:aws:s3:::bucket-name/*"
}
```

## Design Decisions

1. **Streaming over buffering**: Data never touches disk. HTTP chunks flow directly through parser to S3.

2. **On-the-fly filtering**: Wind filtering happens during streaming, not after download. Reduces bandwidth and storage by 95%.

3. **Multipart uploads**: 5 MB parts uploaded as available. Enables reliable uploads and recovery (automatic abort on failure).

4. **Error isolation**: Errors in one file don't stop batch processing. Each date/hour processed independently.

5. **Rust + Python**: Core pipeline in Rust for performance. Python utility for accessible post-processing and analysis.

## Python Utility

`process_wind_data.py` provides post-processing functions:

- `read_wind_data()`: Extract U/V components, coordinates, metadata
- `calculate_wind_stats()`: Compute speed and direction
- `plot_wind_field()`: Contour + quiver visualization
- `analyze_wind_data()`: Statistical summary
- `extract_regional_data()`: Geographic subsetting

## Development

### With Nix

```bash
nix develop  # or: direnv allow
```

Provides: Rust toolchain, cargo-watch, rust-analyzer, clippy

### Manual

```bash
cargo build
cargo run -- --start-date 2020-01-01 --end-date 2020-01-01 --bucket test
cargo test
cargo clippy
```
