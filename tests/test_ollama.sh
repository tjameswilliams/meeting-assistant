#!/bin/bash

# Test script for Ollama provider functionality
# This script runs various tests to ensure the Ollama provider is working correctly

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

print_step() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

check_command() {
    if command -v "$1" &> /dev/null; then
        return 0
    else
        return 1
    fi
}

check_ollama_running() {
    if check_command ollama; then
        if ollama list &> /dev/null; then
            return 0
        else
            return 1
        fi
    else
        return 1
    fi
}

run_unit_tests() {
    print_header "Running Unit Tests"
    
    print_info "Running basic unit tests (no Ollama required)..."
    cargo test plugins::ollama_provider::tests::test_ollama_config_default -- --nocapture
    cargo test plugins::ollama_provider::tests::test_ollama_provider_creation -- --nocapture
    cargo test plugins::ollama_provider::tests::test_plugin_interface -- --nocapture
    cargo test plugins::ollama_provider::tests::test_config_validation -- --nocapture
    cargo test plugins::ollama_provider::tests::test_ollama_options_creation -- --nocapture
    cargo test plugins::ollama_provider::tests::test_health_status -- --nocapture
    cargo test plugins::ollama_provider::tests::test_config_schema -- --nocapture
    cargo test plugins::ollama_provider::tests::test_subscribed_events -- --nocapture
    cargo test plugins::ollama_provider::tests::test_config_serialization -- --nocapture
    
    print_step "Unit tests completed"
}

run_mock_tests() {
    print_header "Running Mock Server Tests"
    
    print_info "Running tests with mock HTTP server..."
    cargo test plugins::ollama_provider::tests::test_with_mock_server -- --nocapture
    
    print_step "Mock server tests completed"
}

run_integration_tests() {
    print_header "Running Integration Tests"
    
    if check_ollama_running; then
        print_info "Ollama is running, running integration tests..."
        
        # Show available models
        print_info "Available Ollama models:"
        ollama list
        
        # Run integration tests
        cargo test plugins::ollama_provider::tests::test_integration_with_ollama -- --nocapture --ignored
        
        print_step "Integration tests completed"
    else
        print_warning "Ollama is not running or not installed"
        print_info "To run integration tests:"
        print_info "1. Install Ollama: https://ollama.ai"
        print_info "2. Start Ollama: ollama serve"
        print_info "3. Pull a model: ollama pull llama2:7b"
        print_info "4. Run this script again"
    fi
}

run_configuration_tests() {
    print_header "Running Configuration Tests"
    
    print_info "Testing configuration loading and validation..."
    cargo test plugins::ollama_provider::tests::test_initialization_with_context -- --nocapture
    cargo test plugins::ollama_provider::tests::test_initialization_with_custom_config -- --nocapture
    cargo test plugins::ollama_provider::tests::test_event_handling -- --nocapture
    cargo test plugins::ollama_provider::tests::test_cleanup -- --nocapture
    
    print_step "Configuration tests completed"
}

run_model_selection_tests() {
    print_header "Running Model Selection Tests"
    
    print_info "Testing model selection logic..."
    cargo test plugins::ollama_provider::tests::test_model_selection -- --nocapture
    cargo test plugins::ollama_provider::tests::test_invalid_model_selection -- --nocapture
    cargo test plugins::ollama_provider::tests::test_openai_model_filtering -- --nocapture
    
    print_step "Model selection tests completed"
}

run_system_diagnostics() {
    print_header "System Diagnostics"
    
    print_info "Checking system requirements..."
    
    # Check Rust and Cargo
    if check_command cargo; then
        print_step "Rust/Cargo: $(cargo --version)"
    else
        print_error "Rust/Cargo not found"
    fi
    
    # Check Ollama
    if check_command ollama; then
        print_step "Ollama: $(ollama --version 2>/dev/null || echo 'installed')"
        
        if check_ollama_running; then
            print_step "Ollama service: Running"
            
            # List models
            models=$(ollama list 2>/dev/null | tail -n +2 | awk '{print $1}' | grep -v "^$" || echo "")
            if [[ -n "$models" ]]; then
                print_step "Available models:"
                echo "$models" | while read -r model; do
                    echo "  • $model"
                done
            else
                print_warning "No models installed"
                print_info "Install a model with: ollama pull llama2:7b"
            fi
        else
            print_warning "Ollama service not running"
            print_info "Start with: ollama serve"
        fi
    else
        print_warning "Ollama not installed"
        print_info "Install from: https://ollama.ai"
    fi
    
    # Check network connectivity
    if curl -s --connect-timeout 5 http://localhost:11434/api/tags > /dev/null 2>&1; then
        print_step "Ollama API: Accessible"
    else
        print_warning "Ollama API not accessible"
    fi
    
    print_step "System diagnostics completed"
}

run_build_test() {
    print_header "Build Test"
    
    print_info "Building the project..."
    if cargo build --release; then
        print_step "Build successful"
    else
        print_error "Build failed"
        exit 1
    fi
}

run_all_tests() {
    print_header "🦙 Ollama Provider Test Suite"
    
    echo -e "${BLUE}This script tests the Ollama provider functionality${NC}"
    echo -e "${BLUE}including unit tests, mock tests, and integration tests.${NC}"
    echo ""
    
    run_system_diagnostics
    echo ""
    
    run_build_test
    echo ""
    
    run_unit_tests
    echo ""
    
    run_mock_tests
    echo ""
    
    run_configuration_tests
    echo ""
    
    run_model_selection_tests
    echo ""
    
    run_integration_tests
    echo ""
    
    print_header "🎉 Test Suite Complete"
    
    if check_ollama_running; then
        echo -e "${GREEN}✅ All tests completed successfully!${NC}"
        echo -e "${GREEN}✅ Ollama provider is ready to use.${NC}"
    else
        echo -e "${YELLOW}⚠️  Unit tests passed, but integration tests were skipped.${NC}"
        echo -e "${YELLOW}   Install and start Ollama to run full integration tests.${NC}"
    fi
}

# Main execution
case "${1:-all}" in
    "unit")
        run_unit_tests
        ;;
    "mock")
        run_mock_tests
        ;;
    "integration")
        run_integration_tests
        ;;
    "config")
        run_configuration_tests
        ;;
    "models")
        run_model_selection_tests
        ;;
    "diagnostics")
        run_system_diagnostics
        ;;
    "build")
        run_build_test
        ;;
    "all")
        run_all_tests
        ;;
    *)
        echo "Usage: $0 [unit|mock|integration|config|models|diagnostics|build|all]"
        echo ""
        echo "  unit         - Run unit tests only"
        echo "  mock         - Run mock server tests"
        echo "  integration  - Run integration tests (requires Ollama)"
        echo "  config       - Run configuration tests"
        echo "  models       - Run model selection tests"
        echo "  diagnostics  - Run system diagnostics"
        echo "  build        - Build the project"
        echo "  all          - Run all tests (default)"
        exit 1
        ;;
esac 