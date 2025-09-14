# Raven Dashboard

Real-time market data visualization and analysis platform built with Dash and Plotly.

## Features

- **Real-time Updates**: 5-second refresh interval for live market data
- **Multi-page Interface**: Organized sections for different types of analysis
- **Interactive Charts**: Powered by Plotly for responsive visualizations
- **InfluxDB Integration**: Direct connection to time-series database

## Pages

### Home
- Market overview with key metrics
- Real-time price and volume charts
- System health monitoring

### Crypto 🪙
- Cryptocurrency-specific analysis
- Price charts (line and candlestick)
- Technical indicators (SMA 10/20)
- Bid-ask spread analysis
- Market metrics (price change, volume, spread)

### Macro 📊
- Macroeconomic analysis
- Asset correlation matrix
- Performance comparison across instruments
- Volatility analysis
- Market statistics

### Analysis 🔬
- Advanced analysis features (placeholder)
- Custom query interface for InfluxDB
- Statistical analysis tools
- Risk analysis capabilities

## Architecture

```
dashboard/
├── app.py              # Main Dash application
├── pages/              # Page modules
│   ├── __init__.py
│   ├── crypto.py       # Cryptocurrency analysis
│   ├── macro.py        # Macroeconomic analysis
│   └── analysis.py     # Advanced analysis
├── utils/              # Utility modules
│   ├── __init__.py
│   └── influx_client.py # InfluxDB connection handler
├── static/             # Static assets
│   └── css/
│       └── styles.css  # Dashboard styling
├── requirements.txt    # Python dependencies
└── README.md          # This file
```

## Environment Variables

- `INFLUX_URL`: InfluxDB URL (default: http://influxdb:8086)
- `INFLUX_TOKEN`: InfluxDB authentication token
- `INFLUX_ORG`: InfluxDB organization (default: house_raven)
- `INFLUX_BUCKET`: InfluxDB bucket name (default: market_data)

## Development

### Local Development

```bash
cd dashboard
pip install -r requirements.txt
python app.py
```

The dashboard will be available at http://localhost:8050

### Docker Development

```bash
# From project root
docker-compose -f docker/docker-compose.yml up dashboard
```

### Production

The dashboard runs with Gunicorn in production:

```bash
gunicorn --bind 0.0.0.0:8050 --workers 2 --timeout 120 app:app.server
```

## Data Requirements

The dashboard expects InfluxDB data with the following structure:

### Market Data Measurement

```
market_data,symbol=BTC-USD price=50000.0,volume=1000.0,bid=49999.0,ask=50001.0 1640995200000000000
```

Fields:
- `price`: Current price
- `volume`: Trading volume
- `bid`: Bid price
- `ask`: Ask price
- `high`: High price (optional)
- `low`: Low price (optional)
- `open`: Opening price (optional)
- `close`: Closing price (optional)

Tags:
- `symbol`: Trading pair symbol

### System Metrics Measurement

```
system_metrics cpu_usage=75.5,memory_usage=60.2 1640995200000000000
```

Fields:
- `cpu_usage`: CPU usage percentage
- `memory_usage`: Memory usage percentage
- Additional system metrics as needed

## Customization

### Adding New Pages

1. Create a new module in `pages/`
2. Implement a `layout()` function returning `html.Div`
3. Add callbacks for interactivity
4. Update `app.py` routing in `display_page()` callback

### Styling

CSS is organized in `static/css/styles.css` with classes:
- `.nav-container`, `.nav-link`: Navigation styling
- `.status-card`, `.metric-card`: Card layouts
- `.chart-container`: Chart wrapper styling
- `.control-panel`, `.control-group`: Control styling

### Adding Charts

Use the `utils/influx_client.py` for data queries and Plotly for visualizations:

```python
from utils.influx_client import InfluxDBHandler
import plotly.graph_objects as go

influx_handler = InfluxDBHandler()
data = influx_handler.query_market_data(time_range="1h")

fig = go.Figure()
fig.add_trace(go.Scatter(x=data['time'], y=data['price']))
```