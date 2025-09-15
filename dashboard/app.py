import streamlit as st
import time
from utils.home.data_ops import get_market_data, get_system_metrics, calculate_market_summary, get_active_symbols_text, prepare_chart_data
from utils.home.charting import create_price_chart, create_volume_chart
from utils.home.templates import (
    load_css, render_page_header, render_section_header, render_metric_card,
    render_colored_metric_card, render_error_message, render_info_message
)

# Page configuration
st.set_page_config(
    page_title="🐦‍⬛ Raven Market Data Dashboard",
    page_icon="🐦‍⬛",
    layout="wide",
    initial_sidebar_state="expanded"
)

# Load CSS styles
load_css()

# Sidebar navigation
st.sidebar.title("🐦‍⬛ Raven Dashboard")
page = st.sidebar.selectbox(
    "Navigate to:",
    ["Home", "Crypto", "Macro", "Analysis"]
)

# Auto-refresh toggle
auto_refresh = st.sidebar.checkbox("Auto-refresh (5s)", value=True)

# Home page
def home_page():
    st.markdown(render_page_header('🐦‍⬛ Raven Market Data Dashboard'), unsafe_allow_html=True)
    st.markdown(render_section_header('Market Data Overview'), unsafe_allow_html=True)

    # Get data
    market_data = get_market_data()
    system_metrics = get_system_metrics()

    # System status section
    st.markdown("#### System Status")
    if system_metrics:
        cols = st.columns(len(system_metrics))
        for i, (key, value) in enumerate(system_metrics.items()):
            with cols[i]:
                formatted_value = f"{value:.2f}" if isinstance(value, float) else str(value)
                st.markdown(render_metric_card(
                    value=formatted_value,
                    label=key.replace('_', ' ').title()
                ), unsafe_allow_html=True)
    else:
        st.markdown(render_error_message('No system data available'), unsafe_allow_html=True)

    # Market summary section
    st.markdown("#### Market Summary")
    market_summary = calculate_market_summary(market_data)

    if market_summary:
        col1, col2, col3, col4 = st.columns(4)

        with col1:
            st.markdown(render_metric_card(
                value=market_summary['total_records'],
                label='Total Records'
            ), unsafe_allow_html=True)

        with col2:
            st.markdown(render_metric_card(
                value=f"{market_summary['avg_volume']:.2f}",
                label='Avg Volume'
            ), unsafe_allow_html=True)

        with col3:
            if market_summary['price_change'] != 0:
                st.markdown(render_colored_metric_card(
                    value=f"{market_summary['price_change']:+.2f}%",
                    label='Price Change',
                    color_class=market_summary['price_change_class']
                ), unsafe_allow_html=True)
            else:
                st.markdown(render_metric_card(
                    value='N/A',
                    label='Price Change'
                ), unsafe_allow_html=True)

        with col4:
            if market_summary['unique_symbols'] > 0:
                st.markdown(render_metric_card(
                    value=market_summary['unique_symbols'],
                    label='Active Symbols'
                ), unsafe_allow_html=True)
            else:
                st.markdown(render_metric_card(
                    value='N/A',
                    label='Active Symbols'
                ), unsafe_allow_html=True)
    else:
        st.markdown(render_error_message('No market data available'), unsafe_allow_html=True)

    # Active symbols section
    symbols_text = get_active_symbols_text(market_data)
    if symbols_text:
        st.markdown("#### Active Symbols")
        st.markdown(render_info_message(symbols_text), unsafe_allow_html=True)

    # Charts section
    st.markdown("#### Charts")

    # Prepare data for charting
    chart_data = prepare_chart_data(market_data)

    # Price chart
    if chart_data is not None and 'price' in chart_data.columns:
        fig_price = create_price_chart(chart_data)
        st.plotly_chart(fig_price, use_container_width=True)
    else:
        st.markdown(render_info_message('No price data available for chart'), unsafe_allow_html=True)

    # Volume chart
    if chart_data is not None and 'volume' in chart_data.columns:
        fig_volume = create_volume_chart(chart_data)
        st.plotly_chart(fig_volume, use_container_width=True)
    else:
        st.markdown(render_info_message('No volume data available for chart'), unsafe_allow_html=True)

# Crypto page
def crypto_page():
    st.markdown(render_page_header('🪙 Cryptocurrency Data'), unsafe_allow_html=True)
    try:
        from pages.crypto import layout
        layout()
    except ImportError:
        st.markdown(render_error_message('Crypto page module not found. Please implement pages/crypto.py'), unsafe_allow_html=True)

# Macro page
def macro_page():
    st.markdown(render_page_header('📈 Macro Economic Data'), unsafe_allow_html=True)
    try:
        from pages.macro import layout
        layout()
    except ImportError:
        st.markdown(render_error_message('Macro page module not found. Please implement pages/macro.py'), unsafe_allow_html=True)

# Analysis page
def analysis_page():
    st.markdown(render_page_header('📊 Data Analysis'), unsafe_allow_html=True)
    try:
        from pages.analysis import layout
        layout()
    except ImportError:
        st.markdown(render_error_message('Analysis page module not found. Please implement pages/analysis.py'), unsafe_allow_html=True)

# Page routing
if page == "Home":
    home_page()
elif page == "Crypto":
    crypto_page()
elif page == "Macro":
    macro_page()
elif page == "Analysis":
    analysis_page()

# Auto-refresh functionality
if auto_refresh:
    time.sleep(5)
    st.rerun()