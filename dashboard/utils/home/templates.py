import streamlit as st

def load_css():
    """Load custom CSS styles"""
    with open('/Users/goonzard/Developer/raven2/dashboard/static/css/styles.css', 'r') as f:
        css = f.read()
    st.markdown(f'<style>{css}</style>', unsafe_allow_html=True)

def load_html_template(template_name):
    """Load an HTML template by name"""
    with open(f'/Users/goonzard/Developer/raven2/dashboard/static/html/{template_name}.html', 'r') as f:
        return f.read()

def render_metric_card(value, label, template_type='metric_card'):
    """Render a metric card with the given value and label"""
    template = load_html_template(template_type)
    return template.format(value=value, label=label)

def render_colored_metric_card(value, label, color_class):
    """Render a colored metric card with the given value, label, and color class"""
    template = load_html_template('metric_card_colored')
    return template.format(value=value, label=label, color_class=color_class)

def render_page_header(title):
    """Render a page header with the given title"""
    template = load_html_template('page_header')
    return template.format(title=title)

def render_section_header(title):
    """Render a section header with the given title"""
    template = load_html_template('section_header')
    return template.format(title=title)

def render_error_message(message):
    """Render an error message with the given text"""
    template = load_html_template('error_message')
    return template.format(message=message)

def render_info_message(message):
    """Render an info message with the given text"""
    template = load_html_template('info_message')
    return template.format(message=message)