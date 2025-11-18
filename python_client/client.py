#!/usr/bin/env python3
"""
Raven Trading Data Client
Real-time gRPC streaming client for market data
"""

import grpc
import time
import uuid
import sys
from datetime import datetime
from typing import List

sys.path.append('generated')

import market_data_pb2
import subscription_pb2
import subscription_pb2_grpc


class RavenClient:
    def __init__(self, server_address: str = "localhost:50051"):
        """Initialize the Raven trading client"""
        self.server_address = server_address
        self.client_id = str(uuid.uuid4())
        self.channel = None
        self.stub = None
        self.running = False
        
    def connect(self):
        """Connect to the gRPC server"""

        self.channel = grpc.insecure_channel(self.server_address)
        self.stub = subscription_pb2_grpc.MarketDataServiceStub(self.channel)
        print(f"Connected to Raven server at {self.server_address}")
        print(f"Client ID: {self.client_id}")
        return True
    
    def disconnect(self):
        """Disconnect from the server"""
        self.running = False
        if self.channel:
            self.channel.close()
            print("Disconnected from server")
    
    def stream_market_data(self, symbols: List[str], duration_seconds: int = 30):
        """Stream real-time market data for specified symbols"""
        if not self.stub:
            print("Not connected to server")
            return
        
        try:
            self.running = True
            
            def request_generator():
                # Send subscription request via stream
                subscribe_req = subscription_pb2.SubscribeRequest(
                    client_id=self.client_id,
                    symbols=symbols,
                    data_types=[
                        subscription_pb2.DataType.TRADES,
                        subscription_pb2.DataType.ORDERBOOK
                    ]
                )
                
                subscription_req = subscription_pb2.SubscriptionRequest(
                    subscribe=subscribe_req
                )
                
                print(f"Subscribing to {len(symbols)} symbols: {', '.join(symbols)}")
                yield subscription_req
                
                # Send initial heartbeat
                time.sleep(0.1)
                heartbeat = subscription_pb2.HeartbeatRequest(
                    client_id=self.client_id,
                    timestamp=int(time.time() * 1000)
                )
                
                yield subscription_pb2.SubscriptionRequest(heartbeat=heartbeat)
                
                # Keep the generator alive by sending periodic heartbeats
                # This prevents the stream from closing
                heartbeat_interval = 10  # Send heartbeat every 10 seconds
                last_heartbeat = time.time()
                
                while self.running:
                    time.sleep(1)  # Check every second
                    
                    current_time = time.time()
                    if current_time - last_heartbeat >= heartbeat_interval:
                        # Send periodic heartbeat
                        heartbeat = subscription_pb2.HeartbeatRequest(
                            client_id=self.client_id,
                            timestamp=int(current_time * 1000)
                        )
                        yield subscription_pb2.SubscriptionRequest(heartbeat=heartbeat)
                        last_heartbeat = current_time
            
            # Start streaming
            stream = self.stub.StreamMarketData(request_generator())
            
            start_time = time.time()
            message_count = 0
            trade_count = 0
            orderbook_count = 0
            
            print(f"Starting {duration_seconds}s market data stream...")
            print("=" * 80)
            
            for message in stream:
                if time.time() - start_time > duration_seconds:
                    print(f"\n{duration_seconds} seconds elapsed, stopping stream...")
                    break
                
                message_count += 1
                timestamp = datetime.now().strftime("%H:%M:%S.%f")[:-3]
                
                if message.HasField('trade'):
                    trade = message.trade
                    trade_count += 1
                    print(f"[{timestamp}] TRADE #{trade_count}: {trade.symbol} - "
                          f"${trade.price:.4f} x {trade.quantity:.4f} ({trade.side})")
                
                elif message.HasField('orderbook'):
                    orderbook = message.orderbook
                    orderbook_count += 1
                    
                    if orderbook.bids and orderbook.asks:
                        best_bid = orderbook.bids[0]
                        best_ask = orderbook.asks[0]
                        spread = best_ask.price - best_bid.price
                        
                        print(f"[{timestamp}] BOOK #{orderbook_count}: {orderbook.symbol} "
                              f"(seq:{orderbook.sequence})")
                        print(f"    Bid: ${best_bid.price:.4f} x {best_bid.quantity:.4f}")
                        print(f"    Ask: ${best_ask.price:.4f} x {best_ask.quantity:.4f}")
                        print(f"    Spread: ${spread:.4f}")
            
            # Summary
            total_time = time.time() - start_time
            rate = message_count / total_time if total_time > 0 else 0
            
            print("=" * 80)
            print("STREAMING SUMMARY:")
            print(f"   Duration: {total_time:.2f} seconds")
            print(f"   Total Messages: {message_count}")
            print(f"   Trades: {trade_count}")
            print(f"   Orderbook Updates: {orderbook_count}")
            print(f"   Message Rate: {rate:.2f} msg/sec")
            
            if message_count > 10:
                print("Real-time streaming successful!")
            else:
                print("Low message count - check server connection")
                
        except grpc.RpcError as e:
            print(f"gRPC Error: {e}")
        except KeyboardInterrupt:
            print("\nStream stopped by user")
        except Exception as e:
            print(f"Error: {e}")
        finally:
            self.running = False


def main():
    """Main function"""
    client = RavenClient()
    
    try:
        # Connect to server
        if not client.connect():
            return
        
        # Stream popular trading pairs for 30 seconds
        symbols = ["BTCUSDT", "ETHUSDT", "ADAUSDT"]
        client.stream_market_data(symbols, duration_seconds=30)
        
    except KeyboardInterrupt:
        print("\nGoodbye!")
    finally:
        client.disconnect()


if __name__ == "__main__":
    main()