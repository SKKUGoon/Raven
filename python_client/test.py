#!/usr/bin/env python3
"""
Quick test script for Raven client
"""

from client import RavenClient

def quick_test():
    """Quick 10-second test"""
    client = RavenClient()
    
    if client.connect():
        print("🧪 Running quick test (10 seconds)...")
        client.stream_market_data(["BTCUSDT"], duration_seconds=10)
        client.disconnect()
    else:
        print("❌ Connection failed")

def full_test():
    """Full 30-second test with multiple symbols"""
    client = RavenClient()
    
    if client.connect():
        print("🚀 Running full test (30 seconds)...")
        symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT"]
        client.stream_market_data(symbols, duration_seconds=30)
        # client.disconnect()
    else:
        print("❌ Connection failed")

if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "quick":
        quick_test()
    else:
        full_test()