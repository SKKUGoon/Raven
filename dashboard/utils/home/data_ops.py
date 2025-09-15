import streamlit as st
import pandas as pd
from utils.influx_client import InfluxDBHandler

# Initialize InfluxDB handler
@st.cache_resource
def init_influx_handler():
    return InfluxDBHandler()

# Data fetching functions
@st.cache_data(ttl=5)
def get_market_data():
    influx_handler = init_influx_handler()
    return influx_handler.query_market_data(time_range="1h")

@st.cache_data(ttl=5)
def get_system_metrics():
    influx_handler = init_influx_handler()
    return influx_handler.query_system_metrics()

def calculate_market_summary(market_data):
    """Calculate market summary metrics from market data"""
    if market_data.empty:
        return None

    summary = {
        'total_records': len(market_data),
        'avg_volume': market_data['volume'].mean() if 'volume' in market_data.columns else 0,
        'price_change': 0,
        'price_change_class': 'neutral',
        'unique_symbols': 0
    }

    # Calculate price change
    if 'price' in market_data.columns and len(market_data) > 1:
        summary['price_change'] = ((market_data['price'].iloc[-1] - market_data['price'].iloc[0]) / market_data['price'].iloc[0] * 100)
        summary['price_change_class'] = 'positive' if summary['price_change'] >= 0 else 'negative'

    # Count unique symbols
    if 'symbol' in market_data.columns:
        summary['unique_symbols'] = market_data['symbol'].nunique()

    return summary

def get_active_symbols_text(market_data, max_symbols=10):
    """Get formatted text for active symbols display"""
    if market_data.empty or 'symbol' not in market_data.columns:
        return None

    unique_symbols = market_data['symbol'].unique()
    symbols_to_show = unique_symbols[:max_symbols]

    symbol_text = ", ".join(symbols_to_show)
    if len(unique_symbols) > max_symbols:
        symbol_text += f" ... and {len(unique_symbols) - max_symbols} more"

    return symbol_text

def prepare_chart_data(market_data):
    """Prepare market data for charting"""
    if market_data.empty:
        return None

    # Convert time column to datetime
    if 'time' in market_data.columns:
        market_data = market_data.copy()
        market_data['time'] = pd.to_datetime(market_data['time'])

    return market_data