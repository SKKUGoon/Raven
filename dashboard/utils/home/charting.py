import plotly.graph_objects as go
import pandas as pd

def create_price_chart(market_data):
    """Create a price chart from market data"""
    if market_data is None or market_data.empty:
        return go.Figure().add_annotation(
            text="No data available",
            x=0.5, y=0.5,
            showarrow=False
        )

    if 'time' not in market_data.columns or 'price' not in market_data.columns:
        return go.Figure().add_annotation(
            text="No price data",
            x=0.5, y=0.5,
            showarrow=False
        )

    fig = go.Figure()

    # Check if we have multiple symbols
    if 'symbol' in market_data.columns and market_data['symbol'].nunique() > 1:
        for symbol in market_data['symbol'].unique():
            symbol_data = market_data[market_data['symbol'] == symbol]
            fig.add_trace(go.Scatter(
                x=symbol_data['time'],
                y=symbol_data['price'],
                mode='lines+markers',
                name=symbol,
                line=dict(width=2)
            ))
    else:
        fig.add_trace(go.Scatter(
            x=market_data['time'],
            y=market_data['price'],
            mode='lines+markers',
            name='Price',
            line=dict(width=2, color='#007bff')
        ))

    fig.update_layout(
        title="Real-time Price Chart",
        xaxis_title="Time",
        yaxis_title="Price",
        height=400,
        hovermode='x unified',
        paper_bgcolor='white',
        plot_bgcolor='white'
    )

    return fig

def create_volume_chart(market_data):
    """Create a volume chart from market data"""
    if market_data is None or market_data.empty:
        return go.Figure().add_annotation(
            text="No data available",
            x=0.5, y=0.5,
            showarrow=False
        )

    if 'time' not in market_data.columns or 'volume' not in market_data.columns:
        return go.Figure().add_annotation(
            text="No volume data",
            x=0.5, y=0.5,
            showarrow=False
        )

    fig = go.Figure()

    # Check if we have multiple symbols
    if 'symbol' in market_data.columns and market_data['symbol'].nunique() > 1:
        for symbol in market_data['symbol'].unique():
            symbol_data = market_data[market_data['symbol'] == symbol]
            fig.add_trace(go.Bar(
                x=symbol_data['time'],
                y=symbol_data['volume'],
                name=symbol,
                opacity=0.7
            ))
    else:
        fig.add_trace(go.Bar(
            x=market_data['time'],
            y=market_data['volume'],
            name='Volume',
            marker_color='#17a2b8',
            opacity=0.7
        ))

    fig.update_layout(
        title="Real-time Volume Chart",
        xaxis_title="Time",
        yaxis_title="Volume",
        height=300,
        showlegend=True,
        paper_bgcolor='white',
        plot_bgcolor='white'
    )

    return fig