# 10_Rust_Axum_WeatherForecastInstaller

A modernized version of the ASP.NET Core WeatherForecast example, rebuilt using **Rust** and the **Axum** web framework.

## Why Rust/Axum?

| Aspect | ASP.NET Core 6 | Rust/Axum |
|--------|----------------|-----------|
| **Runtime dependencies** | .NET Runtime or 300+ self-contained files | Single ~5MB binary |
| **IIS requirement** | Yes (for production hosting) | No - standalone HTTP server |
| **Memory footprint** | ~50-100MB | ~5-10MB |
| **Startup time** | ~500ms | ~10ms |
| **Prerequisites** | .NET SDK, IIS, Hosting Bundle | Rust toolchain (build only) |

## Project Structure

```
10_Rust_Axum_WeatherForecastInstaller/
├── README.md
├── weather-api/                    # Rust web API
│   ├── Cargo.toml
│   └── src/main.rs
└── 10_WeatherForecastInstaller/    # WiX installer
    └── Product.wxs.yaml
```

## Building the Weather API

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (1.70+)

### Build Commands

```bash
cd weather-api

# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release
```

The release binary will be at `target/release/weather-api.exe` (~5MB).

## Running the API

```bash
# Default port 5000
./weather-api.exe

# Custom port via environment variable
$env:WEATHER_API_PORT = "8080"
./weather-api.exe
```

### Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Health check / welcome message |
| `/weatherforecast` | GET | Returns 5 days of random weather forecasts |

### Example Response

```json
[
  {
    "date": "2024-01-15",
    "temperatureC": 22,
    "temperatureF": 71,
    "summary": "Mild"
  },
  {
    "date": "2024-01-16",
    "temperatureC": -5,
    "temperatureF": 23,
    "summary": "Freezing"
  }
]
```

## Building the Installer

### Prerequisites

- [WiX Toolset v4](https://wixtoolset.org/)
- [Wixer](https://github.com/natyamatsya/wixer)

### Build Commands

```bash
# First, build the Rust binary
cd weather-api
cargo build --release
cd ..

# Then build the MSI
wixer build 10_WeatherForecastInstaller/Product.wxs.yaml -d BinarySource=weather-api/target/release
```

## Comparison with ASP.NET Version

### Installer Complexity

| ASP.NET Version | Rust Version |
|-----------------|--------------|
| `Product.wxs.yaml` (75 lines) | `Product.wxs.yaml` (40 lines) |
| `IISHostingFragment.wxs.yaml` (35 lines) | Not needed |
| `WeatherForecastGenerated.wxs.yaml` (~100KB) | Not needed |
| PowerShell custom action for AppPool | Not needed |
| `icacls` permission grants | Not needed |

### What's Eliminated

1. **No IIS configuration** - Axum includes its own async HTTP server (Tokio-based)
2. **No harvesting** - Single binary, no need for `heat.exe`
3. **No runtime prerequisites** - No .NET hosting bundle required
4. **No custom actions** - No AppPool configuration needed
5. **No permission grants** - Runs as a standalone process

## Running as a Windows Service

To run the Weather API as a Windows Service, you can use [NSSM](https://nssm.cc/) or add native Windows Service support:

```bash
# Using NSSM (Non-Sucking Service Manager)
nssm install WeatherForecastAPI "C:\Program Files\WeatherForecastAPI\weather-api.exe"
nssm set WeatherForecastAPI AppEnvironmentExtra WEATHER_API_PORT=5000
nssm start WeatherForecastAPI
```

Alternatively, add the `windows-service` crate to the Rust project for native service support.

## API Compatibility

The JSON response format is identical to the ASP.NET Core template:
- Same field names (`date`, `temperatureC`, `temperatureF`, `summary`)
- Same camelCase serialization
- Same summary strings ("Freezing", "Bracing", "Chilly", etc.)

This makes it a drop-in replacement for any client consuming the original API.
