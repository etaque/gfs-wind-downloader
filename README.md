# GFS Wind Data Downloader

A Rust application that downloads GFS wind data from NCAR RDA, filters for wind variables (UGRD/VGRD) on-the-fly, and streams directly to S3 - without storing anything locally.

## Features

- Streaming architecture: downloads, filters, and uploads in one pass
- No local storage required
- Filters only wind variables (UGRD/VGRD), reducing data by ~95%
- S3 multipart upload for reliable large file transfers

## Prerequisites

1. **Rust**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **AWS Credentials** configured via:
   - Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
   - AWS profile (`AWS_PROFILE`)
   - IAM role (if running on EC2/ECS/Lambda)

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

| Parameter | Description | Default |
|-----------|-------------|---------|
| `--start-date` | Start date (YYYY-MM-DD) | Required |
| `--end-date` | End date (YYYY-MM-DD) | Required |
| `--bucket` | S3 bucket name | Required |
| `--prefix` | S3 key prefix | Empty |

### Output

Files are uploaded to S3 with the pattern:
```
s3://<bucket>/<prefix>/wind_YYYYMMDD_HH.grb2
```

For example:
```
s3://my-gfs-bucket/wind/2020/wind_20200101_00.grb2
s3://my-gfs-bucket/wind/2020/wind_20200101_06.grb2
s3://my-gfs-bucket/wind/2020/wind_20200101_12.grb2
s3://my-gfs-bucket/wind/2020/wind_20200101_18.grb2
```

## Data Information

**Source:** NCEP GFS 0.25 Degree Global Forecast Grids (NCAR RDA ds084.1)
**Resolution:** 0.25 x 0.25 degrees (~25 km at equator)
**Temporal:** 6-hourly (00, 06, 12, 18 UTC)

**Variables Extracted:**
- UGRD: U-component of wind (m/s) - eastward
- VGRD: V-component of wind (m/s) - northward

**Levels:** All available levels including:
- Surface (10 m above ground)
- Pressure levels: 1000, 975, 950, 925, 900, 850, 800, 750, 700, 650, 600, 550, 500, 450, 400, 350, 300, 250, 200, 150, 100 mb and more

## Reading the Data

### Python

```python
import boto3
import pygrib

# Download from S3
s3 = boto3.client('s3')
s3.download_file('my-bucket', 'wind/2020/wind_20200101_00.grb2', '/tmp/wind.grb2')

# Read with pygrib
grbs = pygrib.open('/tmp/wind.grb2')
u_wind = grbs.select(name='U component of wind')[0]
v_wind = grbs.select(name='V component of wind')[0]

u_data = u_wind.values
v_data = v_wind.values
lats, lons = u_wind.latlons()

# Calculate wind speed
wind_speed = (u_data**2 + v_data**2)**0.5
```

### wgrib2

```bash
# Download and inspect
aws s3 cp s3://my-bucket/wind/2020/wind_20200101_00.grb2 - | wgrib2 - -s

# Extract specific level to CSV
aws s3 cp s3://my-bucket/wind/2020/wind_20200101_00.grb2 - | \
  wgrib2 - -match ':UGRD:10 m above ground:' -csv output.csv
```

## Architecture

```
NCAR RDA Server
     |
     | HTTP Stream
     v
+------------------+
| GRIB2 Parser     |  <- Scans for "GRIB" magic, parses message length
| (streaming)      |  <- Filters UGRD/VGRD messages only
+------------------+
     |
     | Filtered messages (~5MB buffer)
     v
+------------------+
| S3 Multipart     |  <- Uploads parts as buffer fills
| Upload           |  <- Completes upload when done
+------------------+
     |
     v
   S3 Bucket
```

## Troubleshooting

### Download Failures
- Some dates may not have data available
- Network timeouts: default is 600s
- Check NCAR RDA status: https://rda.ucar.edu/

### S3 Upload Failures
- Verify AWS credentials are configured
- Check bucket permissions (s3:PutObject, s3:CreateMultipartUpload, etc.)
- Incomplete uploads are automatically aborted on error

## References

- NCAR RDA Dataset: https://rda.ucar.edu/datasets/d084001/
- GFS Documentation: https://www.emc.ncep.noaa.gov/emc/pages/numerical_forecast_systems/gfs.php
