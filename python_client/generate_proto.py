#!/usr/bin/env python3
"""
Script to generate Python gRPC stubs from proto files
"""

import subprocess
import sys
from pathlib import Path

def generate_proto_files():
    """Generate Python gRPC stubs from proto files"""
    
    # Create output directory
    output_dir = Path("generated")
    output_dir.mkdir(exist_ok=True)
    
    # Proto files to compile
    proto_files = [
        "../proto/market_data.proto",
        "../proto/subscription.proto"
    ]
    
    # Generate Python stubs
    cmd = [
        sys.executable, "-m", "grpc_tools.protoc",
        "--proto_path=../proto",
        "--python_out=generated",
        "--grpc_python_out=generated"
    ] + proto_files
    
    print(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, capture_output=True, text=True)
    
    if result.returncode != 0:
        print(f"Error generating proto files: {result.stderr}")
        return False
    
    print("Successfully generated Python gRPC stubs!")
    return True

if __name__ == "__main__":
    generate_proto_files()