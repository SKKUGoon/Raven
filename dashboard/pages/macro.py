from dash import dcc, html, Input, Output, callback
import plotly.graph_objects as go
import pandas as pd
from typing import List, Dict, Any


def layout() -> html.Div:
    """Macro economics page layout"""
    return html.Div([
        html.H2("📊 Macroeconomic Analysis", className='section-header'),

        # Controls
        html.Div([
            html.Div([
                html.Label("Time Range:", className='control-label'),
                dcc.Dropdown(
                    id='macro-time-range',
                    options=[
                        {'label': '1 day', 'value': '1d'},
                        {'label': '7 days', 'value': '7d'},
                        {'label': '30 days', 'value': '30d'},
                        {'label': '90 days', 'value': '90d'}
                    ],
                    value='7d'
                )
            ], className='control-group', style={'width': '30%', 'display': 'inline-block', 'marginRight': '5%'}),

            html.Div([
                html.Label("Analysis Type:", className='control-label'),
                dcc.RadioItems(
                    id='macro-analysis-type',
                    options=[
                        {'label': 'Market Overview', 'value': 'overview'},
                        {'label': 'Correlation Analysis', 'value': 'correlation'},
                        {'label': 'Volatility Analysis', 'value': 'volatility'}
                    ],
                    value='overview',
                    inline=True
                )
            ], className='control-group', style={'width': '60%', 'display': 'inline-block'}),
        ], className='control-panel'),

        # Market overview section
        html.Div(id='macro-overview-section'),

        # Correlation matrix
        html.Div([
            dcc.Graph(id='macro-correlation-heatmap')
        ], className='chart-container'),

        # Market performance comparison
        html.Div([
            html.Div([
                dcc.Graph(id='macro-performance-chart')
            ], className='chart-container', style={'width': '65%', 'display': 'inline-block'}),

            html.Div([
                html.H4("Market Statistics", style={'textAlign': 'center'}),
                html.Div(id='macro-statistics')
            ], className='status-card', style={'width': '30%', 'display': 'inline-block', 'marginLeft': '3%'}),
        ]),

        # Volatility analysis
        html.Div([
            dcc.Graph(id='macro-volatility-chart')
        ], className='chart-container'),
    ])


@callback(
    Output('macro-overview-section', 'children'),
    [Input('market-data-store', 'data'),
     Input('macro-analysis-type', 'value')]
)
def update_macro_overview(market_data: List[Dict[str, Any]], analysis_type: str) -> List[html.Div]:
    """Update macro overview section"""
    if not market_data:
        return [html.P("No market data available", className='error-message')]

    df = pd.DataFrame(market_data)
    if df.empty:
        return [html.P("No market data available", className='error-message')]

    if analysis_type == 'overview':
        # Market overview metrics
        total_symbols = df['symbol'].nunique() if 'symbol' in df.columns else 0
        total_volume = df['volume'].sum() if 'volume' in df.columns else 0
        avg_price = df['price'].mean() if 'price' in df.columns else 0

        return [
            html.Div([
                html.H3("Market Overview", className='section-header'),
                html.Div([
                    html.Div([
                        html.P(f"{total_symbols}", className='metric-value'),
                        html.P("Active Markets", className='metric-label')
                    ], className='metric-card', style={'width': '30%', 'display': 'inline-block', 'margin': '1%'}),

                    html.Div([
                        html.P(f"{total_volume:.2f}", className='metric-value'),
                        html.P("Total Volume", className='metric-label')
                    ], className='metric-card', style={'width': '30%', 'display': 'inline-block', 'margin': '1%'}),

                    html.Div([
                        html.P(f"${avg_price:.2f}", className='metric-value'),
                        html.P("Average Price", className='metric-label')
                    ], className='metric-card', style={'width': '30%', 'display': 'inline-block', 'margin': '1%'}),
                ], style={'textAlign': 'center'})
            ])
        ]

    return []


@callback(
    Output('macro-correlation-heatmap', 'figure'),
    [Input('market-data-store', 'data'),
     Input('macro-time-range', 'value')]
)
def update_correlation_heatmap(market_data: List[Dict[str, Any]], time_range: str) -> go.Figure:
    """Update correlation heatmap"""
    if not market_data:
        return go.Figure().add_annotation(text="No data available", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'symbol' not in df.columns or 'price' not in df.columns:
        return go.Figure().add_annotation(text="Insufficient data for correlation analysis", x=0.5, y=0.5, showarrow=False)

    # Pivot data to get symbols as columns
    df['time'] = pd.to_datetime(df['time'])
    pivot_df = df.pivot_table(
        index='time',
        columns='symbol',
        values='price',
        aggfunc='last'
    ).fillna(method='ffill').dropna()

    if pivot_df.empty or len(pivot_df.columns) < 2:
        return go.Figure().add_annotation(text="Need at least 2 symbols for correlation", x=0.5, y=0.5, showarrow=False)

    # Calculate correlation matrix
    corr_matrix = pivot_df.corr()

    fig = go.Figure(data=go.Heatmap(
        z=corr_matrix.values,
        x=corr_matrix.columns,
        y=corr_matrix.index,
        colorscale='RdBu',
        zmid=0,
        text=corr_matrix.round(3).values,
        texttemplate="%{text}",
        textfont={"size": 10},
        hoverongaps=False
    ))

    fig.update_layout(
        title=f"Asset Correlation Matrix ({time_range})",
        xaxis_title="Asset",
        yaxis_title="Asset",
        height=500,
        paper_bgcolor='white',
        plot_bgcolor='white'
    )

    return fig


@callback(
    Output('macro-performance-chart', 'figure'),
    [Input('market-data-store', 'data'),
     Input('macro-time-range', 'value')]
)
def update_performance_chart(market_data: List[Dict[str, Any]], time_range: str) -> go.Figure:
    """Update performance comparison chart"""
    if not market_data:
        return go.Figure().add_annotation(text="No data available", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'symbol' not in df.columns or 'price' not in df.columns:
        return go.Figure().add_annotation(text="No price data available", x=0.5, y=0.5, showarrow=False)

    df['time'] = pd.to_datetime(df['time'])

    # Calculate normalized performance for each symbol
    fig = go.Figure()

    symbols = df['symbol'].unique()[:10]  # Limit to 10 symbols for clarity

    for symbol in symbols:
        symbol_data = df[df['symbol'] == symbol].sort_values('time')
        if len(symbol_data) > 1:
            # Normalize to percentage change from first price
            first_price = symbol_data['price'].iloc[0]
            symbol_data = symbol_data.copy()
            symbol_data['normalized_price'] = ((symbol_data['price'] - first_price) / first_price) * 100

            fig.add_trace(go.Scatter(
                x=symbol_data['time'],
                y=symbol_data['normalized_price'],
                mode='lines',
                name=symbol,
                line=dict(width=2)
            ))

    fig.update_layout(
        title=f"Normalized Performance Comparison ({time_range})",
        xaxis_title="Time",
        yaxis_title="Performance (%)",
        height=400,
        hovermode='x unified',
        paper_bgcolor='white',
        plot_bgcolor='white',
        showlegend=True
    )

    return fig


@callback(
    Output('macro-statistics', 'children'),
    [Input('market-data-store', 'data'),
     Input('macro-time-range', 'value')]
)
def update_macro_statistics(market_data: List[Dict[str, Any]], time_range: str) -> List[html.Div]:
    """Update macro statistics"""
    if not market_data:
        return [html.P("No data available", className='error-message')]

    df = pd.DataFrame(market_data)
    if df.empty:
        return [html.P("No data available", className='error-message')]

    statistics = []

    # Market cap (approximation)
    if 'price' in df.columns and 'volume' in df.columns:
        market_cap = (df['price'] * df['volume']).sum()
        statistics.append(
            html.Div([
                html.P(f"${market_cap:.2f}", className='metric-value'),
                html.P("Est. Market Cap", className='metric-label')
            ], className='metric-card')
        )

    # Volatility (standard deviation of prices)
    if 'price' in df.columns:
        volatility = df['price'].std()
        statistics.append(
            html.Div([
                html.P(f"{volatility:.4f}", className='metric-value'),
                html.P("Price Volatility", className='metric-label')
            ], className='metric-card')
        )

    # Price range
    if 'price' in df.columns:
        price_range = df['price'].max() - df['price'].min()
        statistics.append(
            html.Div([
                html.P(f"${price_range:.4f}", className='metric-value'),
                html.P("Price Range", className='metric-label')
            ], className='metric-card')
        )

    # Active trading pairs
    unique_symbols = df['symbol'].nunique() if 'symbol' in df.columns else 0
    statistics.append(
        html.Div([
            html.P(f"{unique_symbols}", className='metric-value'),
            html.P("Trading Pairs", className='metric-label')
        ], className='metric-card')
    )

    return statistics


@callback(
    Output('macro-volatility-chart', 'figure'),
    [Input('market-data-store', 'data'),
     Input('macro-time-range', 'value')]
)
def update_volatility_chart(market_data: List[Dict[str, Any]], time_range: str) -> go.Figure:
    """Update volatility analysis chart"""
    if not market_data:
        return go.Figure().add_annotation(text="No data available", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'symbol' not in df.columns or 'price' not in df.columns:
        return go.Figure().add_annotation(text="No price data for volatility analysis", x=0.5, y=0.5, showarrow=False)

    df['time'] = pd.to_datetime(df['time'])

    # Calculate rolling volatility for each symbol
    fig = go.Figure()

    symbols = df['symbol'].unique()[:5]  # Limit to 5 symbols for clarity

    for symbol in symbols:
        symbol_data = df[df['symbol'] == symbol].sort_values('time')
        if len(symbol_data) > 10:
            # Calculate rolling standard deviation as volatility measure
            symbol_data = symbol_data.copy()
            symbol_data['volatility'] = symbol_data['price'].rolling(window=10).std()

            fig.add_trace(go.Scatter(
                x=symbol_data['time'],
                y=symbol_data['volatility'],
                mode='lines',
                name=f'{symbol} Volatility',
                line=dict(width=2)
            ))

    fig.update_layout(
        title=f"Rolling Volatility Analysis ({time_range})",
        xaxis_title="Time",
        yaxis_title="Volatility (Price Std Dev)",
        height=400,
        hovermode='x unified',
        paper_bgcolor='white',
        plot_bgcolor='white',
        showlegend=True
    )

    return fig