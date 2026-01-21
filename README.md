# GFS Wind Data Downloader

A Rust application that downloads GFS wind data from NCAR RDA, filters for wind variables (UGRD/VGRD) on-the-fly, and streams directly to S3 without local storage.

## Features

- Streaming architecture: downloads, filters, and uploads in one pass
- No local storage required
- 95% data reduction by filtering only wind variables
- S3 multipart upload with automatic cleanup on failure

## Prerequisites

- Rust (latest stable)
- AWS credentials configured (env vars, profile, or IAM role)

## Installation

```bash
cargo build --release
```

## Usage

```bash
./target/release/gfs_wind_downloader \
  --start-date 2020-01-01 \
  --end-date 2020-01-07 \
  --bucket my-gfs-bucket \
  --prefix wind/2020/
```

### Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `--start-date` | Yes | Start date (YYYY-MM-DD) |
| `--end-date` | Yes | End date (YYYY-MM-DD) |
| `--bucket` | Yes | S3 bucket name |
| `--prefix` | No | S3 key prefix |
| `--region` | No | AWS region |
| `--endpoint-url` | No | Custom S3 endpoint (for MinIO) |

### Output

Files are uploaded as:
```
s3://<bucket>/<prefix>/wind_YYYYMMDD_HH.grb2
```

## Data Source

- **Source:** NCAR THREDDS server (ds084.1)
- **Variables:** UGRD (U-wind) and VGRD (V-wind) at all levels
- **Temporal:** 6-hourly (00, 06, 12, 18 UTC)
- **Availability:** 2015 to present

## Local Testing with MinIO

Start a local S3-compatible storage:

```bash
docker compose up -d
```

Run the downloader against MinIO:

```bash
export $(cat .env | xargs)
./target/release/gfs_wind_downloader \
  --start-date 2020-01-01 \
  --end-date 2020-01-01 \
  --bucket gfs-wind \
  --endpoint-url http://localhost:9002
```

Access MinIO:
- Console: http://localhost:9003 (minioadmin/minioadmin)
- API: http://localhost:9002

Stop MinIO:

```bash
docker compose down
```

## Reading the Data

```python
import pygrib

grbs = pygrib.open('wind_20200101_00.grb2')
u_wind = grbs.select(name='U component of wind')[0]
v_wind = grbs.select(name='V component of wind')[0]

wind_speed = (u_wind.values**2 + v_wind.values**2)**0.5
```

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Detailed system design and internals

## References

- [NCAR RDA Dataset ds084.1](https://rda.ucar.edu/datasets/d084001/)
- [GFS Documentation](https://www.emc.ncep.noaa.gov/emc/pages/numerical_forecast_systems/gfs.php)
