#!/bin/bash

# Project Raven Deployment Script
# Legacy script - Use Makefile for better experience: make help

set -e

echo "🐦 Project Raven Deployment Script"
echo "=================================="
echo ""
echo "⚠️  This script is deprecated. Please use the Makefile instead:"
echo ""
echo "   make help      # Show all available commands"
echo "   make deploy    # Deploy full stack"
echo "   make status    # Check service status"
echo "   make logs      # Show logs"
echo ""
echo "For more information, run: make help"
echo ""

# Redirect to Makefile if available
if [ -f "../Makefile" ]; then
    echo "🔄 Redirecting to Makefile..."
    cd ..
    case "${1:-help}" in
        "deploy")
            make deploy
            ;;
        "stop")
            make stop
            ;;
        "restart")
            make restart
            ;;
        "status")
            make status
            make urls
            ;;
        "cleanup")
            make clean-all
            ;;
        "backup")
            make backup
            ;;
        "logs")
            if [ -n "$2" ]; then
                make logs-$2
            else
                make logs-server
            fi
            ;;
        *)
            make help
            ;;
    esac
else
    echo "❌ Makefile not found. Please run this script from the project root directory."
    exit 1
fi