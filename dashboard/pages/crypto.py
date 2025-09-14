from dash import dcc, html, Input, Output, callback
import plotly.graph_objects as go
import plotly.express as px
import pandas as pd
from typing import List, Dict, Any


def layout() -> html.Div:
    """Crypto page layout"""
    return html.Div([
        html.H2("🪙 Cryptocurrency Analysis", className='section-header'),

        # Controls
        html.Div([
            html.Div([
                html.Label("Select Crypto Symbol:", className='control-label'),
                dcc.Dropdown(
                    id='crypto-symbol-dropdown',
                    options=[],
                    value=None,
                    placeholder="Select a cryptocurrency..."
                )
            ], className='control-group', style={'width': '30%', 'display': 'inline-block', 'marginRight': '3%'}),

            html.Div([
                html.Label("Time Range:", className='control-label'),
                dcc.Dropdown(
                    id='crypto-time-range',
                    options=[
                        {'label': '15 minutes', 'value': '15m'},
                        {'label': '1 hour', 'value': '1h'},
                        {'label': '4 hours', 'value': '4h'},
                        {'label': '1 day', 'value': '1d'},
                        {'label': '7 days', 'value': '7d'}
                    ],
                    value='4h'
                )
            ], className='control-group', style={'width': '30%', 'display': 'inline-block', 'marginRight': '3%'}),

            html.Div([
                html.Label("Chart Type:", className='control-label'),
                dcc.RadioItems(
                    id='crypto-chart-type',
                    options=[
                        {'label': 'Line', 'value': 'line'},
                        {'label': 'Candlestick', 'value': 'candlestick'}
                    ],
                    value='line',
                    inline=True
                )
            ], className='control-group', style={'width': '30%', 'display': 'inline-block'}),
        ], className='control-panel'),

        # Main charts
        html.Div([
            html.Div([
                dcc.Graph(id='crypto-price-chart')
            ], className='chart-container', style={'width': '65%', 'display': 'inline-block'}),

            html.Div([
                html.H4("Market Metrics", style={'textAlign': 'center'}),
                html.Div(id='crypto-metrics')
            ], className='status-card', style={'width': '30%', 'display': 'inline-block', 'marginLeft': '3%'}),
        ]),

        # Volume and additional charts
        html.Div([
            html.Div([
                dcc.Graph(id='crypto-volume-chart')
            ], className='chart-container', style={'width': '48%', 'display': 'inline-block'}),

            html.Div([
                dcc.Graph(id='crypto-spread-chart')
            ], className='chart-container', style={'width': '48%', 'display': 'inline-block', 'marginLeft': '4%'}),
        ]),

        # Technical indicators
        html.Div([
            html.H3("Technical Indicators", className='section-header'),
            dcc.Graph(id='crypto-technical-chart')
        ], className='chart-container'),
    ])


@callback(
    Output('crypto-symbol-dropdown', 'options'),
    Input('market-data-store', 'data')
)
def update_crypto_symbols(market_data: List[Dict[str, Any]]) -> List[Dict[str, str]]:
    """Update available crypto symbols"""
    if not market_data:
        return []

    df = pd.DataFrame(market_data)
    if df.empty or 'symbol' not in df.columns:
        return []

    # Filter for crypto symbols (you might want to adjust this logic)
    crypto_symbols = df['symbol'].unique()
    crypto_options = [{'label': symbol, 'value': symbol} for symbol in sorted(crypto_symbols)]

    return crypto_options


@callback(
    Output('crypto-price-chart', 'figure'),
    [Input('market-data-store', 'data'),
     Input('crypto-symbol-dropdown', 'value'),
     Input('crypto-time-range', 'value'),
     Input('crypto-chart-type', 'value')]
)
def update_crypto_price_chart(market_data: List[Dict[str, Any]], symbol: str, time_range: str, chart_type: str) -> go.Figure:
    """Update crypto price chart"""
    if not market_data or not symbol:
        return go.Figure().add_annotation(text="Select a cryptocurrency to view data", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty:
        return go.Figure().add_annotation(text="No data available", x=0.5, y=0.5, showarrow=False)

    # Filter for selected symbol
    symbol_data = df[df['symbol'] == symbol] if 'symbol' in df.columns else df
    if symbol_data.empty:
        return go.Figure().add_annotation(text=f"No data for {symbol}", x=0.5, y=0.5, showarrow=False)

    symbol_data['time'] = pd.to_datetime(symbol_data['time'])
    symbol_data = symbol_data.sort_values('time')

    fig = go.Figure()

    if chart_type == 'candlestick' and all(col in symbol_data.columns for col in ['open', 'high', 'low', 'close']):
        fig.add_trace(go.Candlestick(
            x=symbol_data['time'],
            open=symbol_data['open'],
            high=symbol_data['high'],
            low=symbol_data['low'],
            close=symbol_data['close'],
            name=symbol
        ))
    else:
        if 'price' in symbol_data.columns:
            fig.add_trace(go.Scatter(
                x=symbol_data['time'],
                y=symbol_data['price'],
                mode='lines+markers',
                name=f'{symbol} Price',
                line=dict(width=2)
            ))

    fig.update_layout(
        title=f"{symbol} Price Chart ({time_range})",
        xaxis_title="Time",
        yaxis_title="Price (USD)",
        height=500,
        hovermode='x unified',
        paper_bgcolor='white',
        plot_bgcolor='white'
    )

    return fig


@callback(
    Output('crypto-metrics', 'children'),
    [Input('market-data-store', 'data'),
     Input('crypto-symbol-dropdown', 'value')]
)
def update_crypto_metrics(market_data: List[Dict[str, Any]], symbol: str) -> List[html.Div]:
    """Update crypto metrics display"""
    if not market_data or not symbol:
        return [html.P("Select a cryptocurrency", className='info-message')]

    df = pd.DataFrame(market_data)
    if df.empty:
        return [html.P("No data available", className='error-message')]

    symbol_data = df[df['symbol'] == symbol] if 'symbol' in df.columns else df
    if symbol_data.empty:
        return [html.P(f"No data for {symbol}", className='error-message')]

    metrics = []

    # Current price
    if 'price' in symbol_data.columns:
        current_price = symbol_data['price'].iloc[-1]
        metrics.append(
            html.Div([
                html.P(f"${current_price:.4f}", className='metric-value'),
                html.P("Current Price", className='metric-label')
            ], className='metric-card')
        )

    # 24h change
    if 'price' in symbol_data.columns and len(symbol_data) > 1:
        price_change = ((symbol_data['price'].iloc[-1] - symbol_data['price'].iloc[0]) / symbol_data['price'].iloc[0] * 100)
        change_class = 'positive' if price_change >= 0 else 'negative'
        metrics.append(
            html.Div([
                html.P(f"{price_change:+.2f}%", className=f'metric-value {change_class}'),
                html.P("Price Change", className='metric-label')
            ], className='metric-card')
        )

    # Volume
    if 'volume' in symbol_data.columns:
        avg_volume = symbol_data['volume'].mean()
        metrics.append(
            html.Div([
                html.P(f"{avg_volume:.2f}", className='metric-value'),
                html.P("Avg Volume", className='metric-label')
            ], className='metric-card')
        )

    # Spread
    if 'bid' in symbol_data.columns and 'ask' in symbol_data.columns:
        latest_data = symbol_data.iloc[-1]
        spread = latest_data['ask'] - latest_data['bid']
        spread_pct = (spread / latest_data['ask'] * 100) if latest_data['ask'] > 0 else 0
        metrics.append(
            html.Div([
                html.P(f"{spread_pct:.3f}%", className='metric-value'),
                html.P("Bid-Ask Spread", className='metric-label')
            ], className='metric-card')
        )

    return metrics


@callback(
    Output('crypto-volume-chart', 'figure'),
    [Input('market-data-store', 'data'),
     Input('crypto-symbol-dropdown', 'value')]
)
def update_crypto_volume_chart(market_data: List[Dict[str, Any]], symbol: str) -> go.Figure:
    """Update crypto volume chart"""
    if not market_data or not symbol:
        return go.Figure().add_annotation(text="Select a cryptocurrency", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'volume' not in df.columns:
        return go.Figure().add_annotation(text="No volume data", x=0.5, y=0.5, showarrow=False)

    symbol_data = df[df['symbol'] == symbol] if 'symbol' in df.columns else df
    if symbol_data.empty:
        return go.Figure().add_annotation(text=f"No data for {symbol}", x=0.5, y=0.5, showarrow=False)

    symbol_data['time'] = pd.to_datetime(symbol_data['time'])

    fig = go.Figure()
    fig.add_trace(go.Bar(
        x=symbol_data['time'],
        y=symbol_data['volume'],
        name='Volume',
        marker_color='#17a2b8',
        opacity=0.7
    ))

    fig.update_layout(
        title=f"{symbol} Volume",
        xaxis_title="Time",
        yaxis_title="Volume",
        height=300,
        paper_bgcolor='white',
        plot_bgcolor='white'
    )

    return fig


@callback(
    Output('crypto-spread-chart', 'figure'),
    [Input('market-data-store', 'data'),
     Input('crypto-symbol-dropdown', 'value')]
)
def update_crypto_spread_chart(market_data: List[Dict[str, Any]], symbol: str) -> go.Figure:
    """Update crypto bid-ask spread chart"""
    if not market_data or not symbol:
        return go.Figure().add_annotation(text="Select a cryptocurrency", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or not all(col in df.columns for col in ['bid', 'ask']):
        return go.Figure().add_annotation(text="No bid/ask data", x=0.5, y=0.5, showarrow=False)

    symbol_data = df[df['symbol'] == symbol] if 'symbol' in df.columns else df
    if symbol_data.empty:
        return go.Figure().add_annotation(text=f"No data for {symbol}", x=0.5, y=0.5, showarrow=False)

    symbol_data['time'] = pd.to_datetime(symbol_data['time'])
    symbol_data['spread'] = symbol_data['ask'] - symbol_data['bid']

    fig = go.Figure()
    fig.add_trace(go.Scatter(
        x=symbol_data['time'],
        y=symbol_data['spread'],
        mode='lines+markers',
        name='Bid-Ask Spread',
        line=dict(width=2, color='#ffc107')
    ))

    fig.update_layout(
        title=f"{symbol} Bid-Ask Spread",
        xaxis_title="Time",
        yaxis_title="Spread",
        height=300,
        paper_bgcolor='white',
        plot_bgcolor='white'
    )

    return fig


@callback(
    Output('crypto-technical-chart', 'figure'),
    [Input('market-data-store', 'data'),
     Input('crypto-symbol-dropdown', 'value')]
)
def update_crypto_technical_chart(market_data: List[Dict[str, Any]], symbol: str) -> go.Figure:
    """Update technical indicators chart"""
    if not market_data or not symbol:
        return go.Figure().add_annotation(text="Select a cryptocurrency", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'price' not in df.columns:
        return go.Figure().add_annotation(text="No price data for technical analysis", x=0.5, y=0.5, showarrow=False)

    symbol_data = df[df['symbol'] == symbol] if 'symbol' in df.columns else df
    if symbol_data.empty or len(symbol_data) < 20:
        return go.Figure().add_annotation(text="Insufficient data for technical analysis", x=0.5, y=0.5, showarrow=False)

    symbol_data = symbol_data.sort_values('time')
    symbol_data['time'] = pd.to_datetime(symbol_data['time'])

    # Calculate simple moving averages
    symbol_data['SMA_10'] = symbol_data['price'].rolling(window=10).mean()
    symbol_data['SMA_20'] = symbol_data['price'].rolling(window=20).mean()

    fig = go.Figure()

    # Price line
    fig.add_trace(go.Scatter(
        x=symbol_data['time'],
        y=symbol_data['price'],
        mode='lines',
        name='Price',
        line=dict(width=2, color='#007bff')
    ))

    # Moving averages
    fig.add_trace(go.Scatter(
        x=symbol_data['time'],
        y=symbol_data['SMA_10'],
        mode='lines',
        name='SMA 10',
        line=dict(width=1, color='#28a745')
    ))

    fig.add_trace(go.Scatter(
        x=symbol_data['time'],
        y=symbol_data['SMA_20'],
        mode='lines',
        name='SMA 20',
        line=dict(width=1, color='#dc3545')
    ))

    fig.update_layout(
        title=f"{symbol} Technical Analysis",
        xaxis_title="Time",
        yaxis_title="Price",
        height=400,
        paper_bgcolor='white',
        plot_bgcolor='white',
        hovermode='x unified'
    )

    return fig