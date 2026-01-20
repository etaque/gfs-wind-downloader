#!/usr/bin/env python3
"""
Example script to read and process GFS wind data from GRIB2 files
Requires: pip install pygrib numpy matplotlib
"""

import sys
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import pygrib


def read_wind_data(grib_file):
    """
    Read u and v wind components from a GRIB2 file

    Returns:
        dict with keys: u, v, lats, lons, time, level
    """
    print(f"Reading: {grib_file}")

    grbs = pygrib.open(str(grib_file))

    # Get u-component (UGRD)
    u_grb = grbs.select(name="U component of wind")[0]
    u_data = u_grb.values
    lats, lons = u_grb.latlons()

    # Get v-component (VGRD)
    grbs.rewind()
    v_grb = grbs.select(name="V component of wind")[0]
    v_data = v_grb.values

    # Get metadata
    level = u_grb.level
    valid_time = u_grb.validDate

    grbs.close()

    return {
        "u": u_data,
        "v": v_data,
        "lats": lats,
        "lons": lons,
        "time": valid_time,
        "level": level,
    }


def calculate_wind_stats(u, v):
    """Calculate wind speed and direction from u/v components"""
    # Wind speed (m/s)
    speed = np.sqrt(u**2 + v**2)

    # Wind direction (degrees, meteorological convention)
    # 0° = from North, 90° = from East, etc.
    direction = np.arctan2(-u, -v) * 180 / np.pi
    direction = (direction + 360) % 360

    return speed, direction


def plot_wind_field(data, output_file="wind_plot.png"):
    """Create a basic wind field visualization"""
    u = data["u"]
    v = data["v"]
    lats = data["lats"]
    lons = data["lons"]

    speed, _ = calculate_wind_stats(u, v)

    # Subsample for quiver plot (every 20th point)
    step = 20
    u_sub = u[::step, ::step]
    v_sub = v[::step, ::step]
    lats_sub = lats[::step, ::step]
    lons_sub = lons[::step, ::step]
    speed_sub = speed[::step, ::step]

    # Create plot
    fig, ax = plt.subplots(figsize=(14, 8))

    # Plot wind speed as background
    im = ax.contourf(lons, lats, speed, levels=20, cmap="YlOrRd", alpha=0.7)

    # Plot wind vectors
    ax.quiver(
        lons_sub,
        lats_sub,
        u_sub,
        v_sub,
        speed_sub,
        cmap="Blues",
        scale=500,
        width=0.002,
    )

    # Add colorbar
    plt.colorbar(im, ax=ax, label="Wind Speed (m/s)")

    # Labels
    ax.set_xlabel("Longitude")
    ax.set_ylabel("Latitude")
    ax.set_title(f"Wind Field - {data['time']} - Level: {data['level']}")
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_file, dpi=150, bbox_inches="tight")
    print(f"✓ Plot saved: {output_file}")


def analyze_wind_data(data):
    """Print statistics about the wind data"""
    u = data["u"]
    v = data["v"]
    speed, direction = calculate_wind_stats(u, v)

    print("\n" + "=" * 50)
    print("Wind Data Analysis")
    print("=" * 50)
    print(f"Time: {data['time']}")
    print(f"Level: {data['level']}")
    print(f"Grid size: {u.shape}")
    print("\nWind Speed Statistics (m/s):")
    print(f"  Min:  {speed.min():.2f}")
    print(f"  Max:  {speed.max():.2f}")
    print(f"  Mean: {speed.mean():.2f}")
    print(f"  Std:  {speed.std():.2f}")
    print("\nU-component Statistics (m/s):")
    print(f"  Min:  {u.min():.2f}")
    print(f"  Max:  {u.max():.2f}")
    print(f"  Mean: {u.mean():.2f}")
    print("\nV-component Statistics (m/s):")
    print(f"  Min:  {v.min():.2f}")
    print(f"  Max:  {v.max():.2f}")
    print(f"  Mean: {v.mean():.2f}")

    # Find location of maximum wind speed
    max_idx = np.unravel_index(speed.argmax(), speed.shape)
    max_lat = data["lats"][max_idx]
    max_lon = data["lons"][max_idx]
    max_speed = speed[max_idx]
    print("\nMaximum wind speed location:")
    print(f"  Speed: {max_speed:.2f} m/s")
    print(f"  Lat: {max_lat:.2f}°")
    print(f"  Lon: {max_lon:.2f}°")
    print("=" * 50 + "\n")


def extract_regional_data(data, lat_range, lon_range):
    """
    Extract wind data for a specific region

    Args:
        data: Wind data dictionary
        lat_range: tuple (min_lat, max_lat)
        lon_range: tuple (min_lon, max_lon)
    """
    lats = data["lats"]
    lons = data["lons"]

    # Create mask for region
    mask = (
        (lats >= lat_range[0])
        & (lats <= lat_range[1])
        & (lons >= lon_range[0])
        & (lons <= lon_range[1])
    )

    regional_data = {
        "u": data["u"][mask],
        "v": data["v"][mask],
        "lats": lats[mask],
        "lons": lons[mask],
        "time": data["time"],
        "level": data["level"],
    }

    return regional_data


def main():
    if len(sys.argv) < 2:
        print("Usage: python process_wind_data.py <grib_file>")
        print("Example: python process_wind_data.py wind_gfs_20200101_00.grb2")
        sys.exit(1)

    grib_file = Path(sys.argv[1])

    if not grib_file.exists():
        print(f"Error: File not found: {grib_file}")
        sys.exit(1)

    # Read wind data
    data = read_wind_data(grib_file)

    # Analyze data
    analyze_wind_data(data)

    # Create visualization
    output_plot = grib_file.stem + "_plot.png"
    plot_wind_field(data, output_plot)

    # Example: Extract data for Europe
    # europe_data = extract_regional_data(data,
    #                                      lat_range=(35, 70),
    #                                      lon_range=(-10, 40))
    # analyze_wind_data(europe_data)

    print("✓ Processing complete!")


if __name__ == "__main__":
    main()
