import dash
from dash import dcc, html, Input, Output, callback
import plotly.graph_objects as go
import pandas as pd
from typing import List, Dict, Any, Optional
from utils.influx_client import InfluxDBHandler

# Initialize Dash app
app = dash.Dash(__name__,
                suppress_callback_exceptions=True,
                external_stylesheets=['/static/css/styles.css'])
app.title = "Raven Market Data Dashboard"

# Initialize InfluxDB handler
influx_handler = InfluxDBHandler()

# Define the layout
app.layout = html.Div([
    dcc.Location(id='url', refresh=False),
    html.Div([
        html.H1("🐦‍⬛ Raven Market Data Dashboard", className='page-header'),

        # Navigation
        html.Div([
            dcc.Link('Home', href='/', className='nav-link'),
            dcc.Link('Crypto', href='/crypto', className='nav-link'),
            dcc.Link('Macro', href='/macro', className='nav-link'),
            dcc.Link('Analysis', href='/analysis', className='nav-link'),
        ], className='nav-container'),

        # Page content
        html.Div(id='page-content'),

        # Interval component for real-time updates
        dcc.Interval(
            id='interval-component',
            interval=5*1000,  # Update every 5 seconds
            n_intervals=0
        ),

        # Store components for data sharing
        dcc.Store(id='market-data-store'),
        dcc.Store(id='system-metrics-store'),
    ]),
])

# Home page layout
def home_layout():
    return html.Div([
        html.H2("Market Data Overview", className='section-header'),

        html.Div([
            html.Div([
                html.H4("System Status"),
                html.Div(id='system-status', children=[
                    html.P("Loading system metrics...", className='loading-spinner')
                ])
            ], className='status-card', style={'width': '30%', 'display': 'inline-block'}),

            html.Div([
                html.H4("Market Summary"),
                html.Div(id='market-summary', children=[
                    html.P("Loading market data...", className='loading-spinner')
                ])
            ], className='status-card', style={'width': '30%', 'display': 'inline-block'}),

            html.Div([
                html.H4("Active Symbols"),
                html.Div(id='active-symbols', children=[
                    html.P("Loading symbols...", className='loading-spinner')
                ])
            ], className='status-card', style={'width': '30%', 'display': 'inline-block'}),
        ], style={'textAlign': 'center'}),

        html.Div([
            dcc.Graph(id='overview-price-chart'),
        ], className='chart-container'),

        html.Div([
            dcc.Graph(id='overview-volume-chart'),
        ], className='chart-container'),
    ])

# Page routing callback
@callback(Output('page-content', 'children'), Input('url', 'pathname'))
def display_page(pathname):
    if pathname == '/crypto':
        from pages.crypto import layout
        return layout()
    elif pathname == '/macro':
        from pages.macro import layout
        return layout()
    elif pathname == '/analysis':
        from pages.analysis import layout
        return layout()
    else:
        return home_layout()

# Data update callback
@callback(
    [Output('market-data-store', 'data'),
     Output('system-metrics-store', 'data')],
    Input('interval-component', 'n_intervals')
)
def update_data(n: int) -> tuple[List[Dict[str, Any]], Dict[str, Any]]:
    market_data = influx_handler.query_market_data(time_range="1h")
    system_metrics = influx_handler.query_system_metrics()
    return market_data.to_dict('records'), system_metrics

# System status callback
@callback(
    Output('system-status', 'children'),
    Input('system-metrics-store', 'data')
)
def update_system_status(system_data):
    if not system_data:
        return [html.P("No system data", className='error-message')]

    status_items = []
    for key, value in system_data.items():
        formatted_value = f"{value:.2f}" if isinstance(value, float) else str(value)
        status_items.append(
            html.Div([
                html.P(formatted_value, className='metric-value'),
                html.P(key.replace('_', ' ').title(), className='metric-label')
            ], className='metric-card')
        )

    return status_items

# Market summary callback
@callback(
    Output('market-summary', 'children'),
    Input('market-data-store', 'data')
)
def update_market_summary(market_data):
    if not market_data:
        return [html.P("No market data", className='error-message')]

    df = pd.DataFrame(market_data)
    if df.empty:
        return [html.P("No market data", className='error-message')]

    total_records = len(df)
    avg_volume = df['volume'].mean() if 'volume' in df.columns else 0
    price_change = 0

    if 'price' in df.columns and len(df) > 1:
        price_change = ((df['price'].iloc[-1] - df['price'].iloc[0]) / df['price'].iloc[0] * 100)

    change_class = 'positive' if price_change >= 0 else 'negative'

    return [
        html.Div([
            html.P(f"{total_records}", className='metric-value'),
            html.P("Total Records", className='metric-label')
        ], className='metric-card'),
        html.Div([
            html.P(f"{avg_volume:.2f}", className='metric-value'),
            html.P("Avg Volume", className='metric-label')
        ], className='metric-card'),
        html.Div([
            html.P(f"{price_change:+.2f}%", className=f'metric-value {change_class}'),
            html.P("Price Change", className='metric-label')
        ], className='metric-card')
    ]

# Active symbols callback
@callback(
    Output('active-symbols', 'children'),
    Input('market-data-store', 'data')
)
def update_active_symbols(market_data):
    if not market_data:
        return [html.P("No symbols", className='error-message')]

    df = pd.DataFrame(market_data)
    if df.empty or 'symbol' not in df.columns:
        return [html.P("No symbols", className='error-message')]

    unique_symbols = df['symbol'].unique()
    symbol_list = [html.P(symbol, style={'margin': '3px 0'}) for symbol in unique_symbols[:10]]

    if len(unique_symbols) > 10:
        symbol_list.append(html.P(f"... and {len(unique_symbols) - 10} more",
                                 style={'fontStyle': 'italic', 'color': '#6c757d'}))

    return symbol_list

# Overview price chart callback
@callback(
    Output('overview-price-chart', 'figure'),
    Input('market-data-store', 'data')
)
def update_price_chart(market_data):
    if not market_data:
        return go.Figure().add_annotation(text="No data available", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'time' not in df.columns or 'price' not in df.columns:
        return go.Figure().add_annotation(text="No price data", x=0.5, y=0.5, showarrow=False)

    df['time'] = pd.to_datetime(df['time'])

    fig = go.Figure()

    if 'symbol' in df.columns and df['symbol'].nunique() > 1:
        for symbol in df['symbol'].unique():
            symbol_data = df[df['symbol'] == symbol]
            fig.add_trace(go.Scatter(
                x=symbol_data['time'],
                y=symbol_data['price'],
                mode='lines+markers',
                name=symbol,
                line=dict(width=2)
            ))
    else:
        fig.add_trace(go.Scatter(
            x=df['time'],
            y=df['price'],
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

# Overview volume chart callback
@callback(
    Output('overview-volume-chart', 'figure'),
    Input('market-data-store', 'data')
)
def update_volume_chart(market_data):
    if not market_data:
        return go.Figure().add_annotation(text="No data available", x=0.5, y=0.5, showarrow=False)

    df = pd.DataFrame(market_data)
    if df.empty or 'time' not in df.columns or 'volume' not in df.columns:
        return go.Figure().add_annotation(text="No volume data", x=0.5, y=0.5, showarrow=False)

    df['time'] = pd.to_datetime(df['time'])

    fig = go.Figure()

    if 'symbol' in df.columns and df['symbol'].nunique() > 1:
        for symbol in df['symbol'].unique():
            symbol_data = df[df['symbol'] == symbol]
            fig.add_trace(go.Bar(
                x=symbol_data['time'],
                y=symbol_data['volume'],
                name=symbol,
                opacity=0.7
            ))
    else:
        fig.add_trace(go.Bar(
            x=df['time'],
            y=df['volume'],
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

if __name__ == '__main__':
    app.run_server(debug=True, host='0.0.0.0', port=8050)