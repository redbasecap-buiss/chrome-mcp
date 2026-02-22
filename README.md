# chrome-mcp

**Chrome browser automation via MCP – click anywhere**

A Model Context Protocol (MCP) server that provides comprehensive Chrome browser automation capabilities. Unlike traditional browser automation tools that are limited to DOM elements, chrome-mcp can interact with **anything** in the browser – including browser UI, popups, shadow DOM, cross-origin iframes, and system dialogs.

## 🚀 Key Features

- **Click Anywhere**: Not just DOM elements, but browser chrome, extension popups, system dialogs
- **Multi-Strategy Element Finding**: CSS selectors, accessibility tree, text content, visual recognition
- **Native Input Injection**: Direct system-level mouse/keyboard events (macOS)
- **Comprehensive Automation**: Navigation, clicking, typing, scrolling, screenshots, PDFs
- **MCP Protocol**: Standard JSON-RPC over stdio for seamless integration
- **Accessibility-First**: Leverages Chrome's accessibility tree for reliable element targeting

## 🏗️ Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   MCP Client    │◄──►│   chrome-mcp     │◄──►│     Chrome      │
│ (Claude, etc.)  │    │     Server       │    │   (DevTools)    │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │                        
                              ▼                        
                    ┌──────────────────┐               
                    │  Native Input    │               
                    │    (macOS)       │               
                    └──────────────────┘               

Layer 1: CDP (Chrome DevTools Protocol)
├─ WebSocket connection to Chrome
├─ DOM manipulation & JavaScript execution
├─ Network interception & monitoring
└─ Screenshot & PDF generation

Layer 2: Accessibility Tree
├─ Semantic element discovery
├─ Role-based targeting (button, link, etc.)
└─ Text content matching

Layer 3: Native Input (macOS)
├─ Core Graphics event injection
├─ Pixel-coordinate clicking
└─ System-level keyboard/mouse

Layer 4: MCP Protocol
├─ JSON-RPC over stdio
├─ Tool registration & discovery
└─ Standardized client integration
```

## 🛠️ Installation

### Prerequisites

1. **Chrome/Chromium** with DevTools enabled:
   ```bash
   # Start Chrome with remote debugging
   google-chrome --remote-debugging-port=9222 --disable-web-security
   # or Chromium
   chromium --remote-debugging-port=9222 --disable-web-security
   ```

2. **Rust** (for building from source):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Build & Install

```bash
# Clone and build
git clone https://github.com/redbasecap-buiss/chrome-mcp.git
cd chrome-mcp
cargo build --release

# Install globally
cargo install --path .
```

### Quick Test

```bash
# Test connection to Chrome
chrome-mcp --chrome-host localhost --chrome-port 9222
```

## 📋 Available Tools

### Navigation & Page Control
- `chrome_navigate` — Navigate to URL
- `chrome_tabs` — List/create/switch/close tabs
- `chrome_wait` — Wait for conditions (page load, elements, etc.)
- `chrome_evaluate` — Execute JavaScript

### Element Interaction
- `chrome_click` — Click by selector, text, or accessibility label
- `chrome_type` — Type text into elements
- `chrome_hover` — Hover over elements
- `chrome_select` — Select dropdown options
- `chrome_scroll` — Scroll page or to elements

### Advanced Clicking
- `chrome_native_click` — Click at screen coordinates (browser UI)
- `chrome_find` — Find elements with detailed references

### Capture & Export
- `chrome_screenshot` — Page/element screenshots (PNG/JPEG)
- `chrome_pdf` — Generate PDFs with options

### Data & State
- `chrome_cookies` — Get/set/clear cookies
- `chrome_accessibility_tree` — Inspect accessibility tree

### Network & Debugging
- `chrome_network` — Monitor/intercept requests (coming soon)

## 🔧 Configuration

### MCP Client Setup

#### Claude Desktop
Add to your Claude Desktop configuration:

```json
{
  "mcpServers": {
    "chrome-automation": {
      "command": "chrome-mcp",
      "args": ["--chrome-host", "localhost", "--chrome-port", "9222"],
      "env": {}
    }
  }
}
```

#### Other MCP Clients
chrome-mcp follows the standard MCP protocol over stdio. Configuration will vary by client.

### Chrome Setup

#### Basic Setup
```bash
google-chrome --remote-debugging-port=9222
```

#### Development Setup (Less Secure)
```bash
google-chrome \
  --remote-debugging-port=9222 \
  --disable-web-security \
  --disable-features=VizDisplayCompositor \
  --user-data-dir=/tmp/chrome-dev
```

#### Headless Mode
```bash
google-chrome --headless --remote-debugging-port=9222
```

## 📚 Usage Examples

### Basic Navigation & Interaction
```typescript
// Navigate to a page
await mcp.call('chrome_navigate', { url: 'https://github.com' });

// Click the sign in button
await mcp.call('chrome_click', { target: 'Sign in' });

// Type in username field
await mcp.call('chrome_type', { 
  text: 'myusername',
  selector: 'input[name="login"]' 
});

// Take a screenshot
const screenshot = await mcp.call('chrome_screenshot', { 
  format: 'png',
  full_page: true 
});
```

### Advanced Element Finding
```typescript
// Find elements by various strategies
const elements = await mcp.call('chrome_find', { 
  query: 'Login button' 
});

// Click using accessibility tree
await mcp.call('chrome_click', { target: 'button[role="button"]' });

// Click by text content
await mcp.call('chrome_click', { target: 'Create account' });
```

### Browser UI Automation
```typescript
// Click on browser UI (tabs, address bar, etc.)
await mcp.call('chrome_native_click', { x: 100, y: 50 });

// Handle system dialogs
await mcp.call('chrome_native_click', { x: 500, y: 300 });
```

### Wait for Conditions
```typescript
// Wait for page load
await mcp.call('chrome_wait', { 
  condition: 'page_load',
  timeout: 30000 
});

// Wait for element to appear
await mcp.call('chrome_wait', { 
  condition: 'element_present',
  target: '#dynamic-content',
  timeout: 10000 
});

// Wait for text to be present
await mcp.call('chrome_wait', { 
  condition: 'text_present',
  target: 'Welcome back!',
  timeout: 5000 
});
```

### Data Extraction
```typescript
// Get page accessibility tree
const tree = await mcp.call('chrome_accessibility_tree', { 
  summary: true 
});

// Execute custom JavaScript
const result = await mcp.call('chrome_evaluate', { 
  javascript: 'document.title' 
});

// Get all cookies
const cookies = await mcp.call('chrome_cookies', { action: 'get' });
```

### PDF & Screenshots
```typescript
// Generate PDF
const pdf = await mcp.call('chrome_pdf', {
  landscape: false,
  print_background: true,
  scale: 1.0
});

// Screenshot specific element
const elementScreenshot = await mcp.call('chrome_screenshot', {
  selector: '.main-content',
  format: 'png'
});
```

## 🔍 Element Finding Strategies

chrome-mcp uses multiple strategies to find elements, tried in order:

1. **CSS Selectors**: Standard DOM selectors
   ```typescript
   chrome_click({ target: '#submit-button' })
   chrome_click({ target: '.nav-item:first-child' })
   ```

2. **Text Content**: Visible text in elements
   ```typescript
   chrome_click({ target: 'Sign In' })
   chrome_click({ target: 'Create New Account' })
   ```

3. **Accessibility Labels**: ARIA labels and roles
   ```typescript
   chrome_click({ target: 'Search button' }) // aria-label
   chrome_click({ target: 'button' })        // role
   ```

4. **Coordinate Fallback**: For unreachable elements
   ```typescript
   chrome_native_click({ x: 100, y: 200 })
   ```

## 🧪 Testing

Run the comprehensive test suite:

```bash
# Unit tests
cargo test

# Integration tests (requires Chrome)
cargo test --test integration_tests -- --ignored

# Linting
cargo clippy -- -D warnings

# Check formatting
cargo fmt --check
```

## 🚦 Platform Support

| Platform | CDP Support | Native Input | Status |
|----------|-------------|--------------|--------|
| macOS    | ✅          | ✅           | Full   |
| Linux    | ✅          | ❌           | Partial |
| Windows  | ✅          | ❌           | Partial |

*Native input (for browser UI clicking) is currently macOS-only. CDP-based automation works on all platforms.*

## 🔧 Troubleshooting

### Chrome Not Connecting
```bash
# Check if Chrome is running with DevTools
curl http://localhost:9222/json

# Restart Chrome with correct flags
google-chrome --remote-debugging-port=9222
```

### Element Not Found
```typescript
// Try different strategies
await mcp.call('chrome_find', { query: 'your-target' });

// Check accessibility tree
await mcp.call('chrome_accessibility_tree', { summary: true });

// Use coordinates as fallback
await mcp.call('chrome_native_click', { x: 100, y: 200 });
```

### macOS Permissions
If native input fails:
1. System Preferences → Security & Privacy → Accessibility
2. Add your terminal app or chrome-mcp binary
3. Restart chrome-mcp

### Network Issues
```bash
# Check Chrome DevTools port
netstat -an | grep 9222

# Try different port
chrome-mcp --chrome-port 9223
```

## 🤝 Contributing

Contributions welcome! Please read our [Contributing Guide](CONTRIBUTING.md) first.

### Development Setup
```bash
git clone https://github.com/redbasecap-buiss/chrome-mcp.git
cd chrome-mcp

# Install dev dependencies
cargo install cargo-watch cargo-nextest

# Run tests in watch mode
cargo watch -x test

# Format code
cargo fmt

# Run linters
cargo clippy
```

### Architecture Notes
- **CDP Client**: WebSocket connection to Chrome DevTools
- **Browser Layer**: High-level automation interface
- **MCP Server**: JSON-RPC protocol implementation
- **Native Input**: Platform-specific input injection
- **Error Handling**: Comprehensive error types with context

## 📄 License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## 🔗 Links

- **GitHub**: https://github.com/redbasecap-buiss/chrome-mcp
- **MCP Protocol**: https://modelcontextprotocol.io
- **Chrome DevTools**: https://chromedevtools.github.io/devtools-protocol/

---

**Made with ❤️ for the MCP community**