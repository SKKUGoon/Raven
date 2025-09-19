# Raven Python Client

High-performance Python gRPC client for streaming real-time cryptocurrency trading data from the Raven market data server.

## 🚀 Quick Start

1. **Install dependencies:**
   ```bash
   cd python_client
   pip install -r requirements.txt
   ```

2. **Generate gRPC stubs:**
   ```bash
   python generate_proto.py
   ```

3. **Start streaming:**
   ```bash
   python client.py
   ```

## 📋 Usage

### Main Client
Stream market data for 30 seconds (default):
```bash
python client.py
```

### Test Scripts
Quick 10-second test:
```bash
python test.py quick
```

Full 30-second test:
```bash
python test.py
```

## ✨ Features

- **🔥 High-frequency streaming**: 20+ messages per second
- **📊 Multiple data types**: Live trades and orderbook updates
- **💰 Popular symbols**: BTCUSDT, ETHUSDT, ADAUSDT
- **⏱️ Real-time timestamps**: Microsecond precision
- **📈 Live market data**: Prices, volumes, spreads, sequences
- **🛡️ Robust connection**: Automatic heartbeats and cleanup
- **📱 Clean interface**: Beautiful formatted output with emojis

## 📊 Example Output

```
✅ Connected to Raven server at localhost:50051
📱 Client ID: cb2ab848-080b-4682-99e8-68837332cd36
📡 Subscribing to 3 symbols: BTCUSDT, ETHUSDT, ADAUSDT
🚀 Starting 30s market data stream...
================================================================================
[10:55:30.474] 📈 TRADE #168: BTCUSDT - $117168.0000 x 0.0110 (buy)
[10:55:30.575] 📊 BOOK #169: BTCUSDT (seq:7678)
    Bid: $117168.6000 x 2.0030
    Ask: $117168.8000 x 0.0020
    Spread: $0.2000
[10:55:30.677] 📈 TRADE #170: BTCUSDT - $117169.3000 x 0.0040 (buy)
[10:55:30.774] 📊 BOOK #171: BTCUSDT (seq:7680)
    Bid: $117169.8000 x 14.3600
    Ask: $117169.9000 x 0.0700
    Spread: $0.1000
...
================================================================================
📊 STREAMING SUMMARY:
   Duration: 30.08 seconds
   Total Messages: 602
   Trades: 301
   Orderbook Updates: 301
   Message Rate: 20.02 msg/sec
✅ Real-time streaming successful!
🔌 Disconnected from server
```

## 🏗️ Architecture

The client uses **bidirectional gRPC streaming** for optimal performance:

1. **Connection**: Establishes secure gRPC channel to server
2. **Subscription**: Sends subscription requests via stream
3. **Heartbeats**: Maintains connection with periodic heartbeats
4. **Data Flow**: Receives continuous real-time market updates
5. **Cleanup**: Graceful disconnection and resource cleanup

## 📁 Files

| File | Description |
|------|-------------|
| `client.py` | Main streaming client with full functionality |
| `test.py` | Test scripts for quick validation |
| `generate_proto.py` | Generates Python gRPC stubs from proto files |
| `requirements.txt` | Python package dependencies |
| `generated/` | Auto-generated gRPC protocol buffer stubs |

## 🔧 Configuration

Default server: `localhost:50051`

To connect to a different server:
```python
client = RavenClient("your-server:50051")
```

## 🐛 Troubleshooting

**Connection refused:**
- Ensure Raven server is running on port 50051
- Check firewall settings

**Import errors:**
- Run `python generate_proto.py` to regenerate stubs
- Verify all dependencies are installed

**No data received:**
- Check server logs for active data feeds
- Verify symbols are available (BTCUSDT, ETHUSDT, ADAUSDT)

## 📈 Performance

- **Latency**: Sub-millisecond message processing
- **Throughput**: 20+ messages per second sustained
- **Memory**: Minimal footprint with efficient streaming
- **CPU**: Low overhead gRPC implementation