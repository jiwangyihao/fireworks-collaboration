# Proxy Configuration Guide (P5.0-P5.5)

## Overview

Starting from P5.0, the application supports proxy configuration for HTTP/HTTPS and SOCKS5 proxies, including automatic system proxy detection, automatic fallback, and automatic recovery.

**Implementation Status**:
- ✅ **P5.0**: Base architecture, configuration model, state machine, system proxy detection
- ✅ **P5.1**: HTTP/HTTPS proxy with CONNECT tunnel and Basic Auth
- ✅ **P5.2**: SOCKS5 proxy with No Auth and Username/Password Auth
- ✅ **P5.3**: Transport layer integration, Fake SNI exclusion
- ✅ **P5.4**: Automatic fallback with sliding window failure detection
- ✅ **P5.5**: Automatic recovery with health checks and cooldown
- ✅ **P5.6**: Frontend UI, proxy commands, event extensions (COMPLETED)

## Configuration Structure

The proxy configuration is added to the main `config.json` file under the `proxy` key:

```json
{
  "http": { ... },
  "tls": { ... },
  "logging": { ... },
  "retry": { ... },
  "ipPool": { ... },
  "proxy": {
    "mode": "off",
    "url": "",
    "username": null,
    "password": null,
    "disableCustomTransport": false,
    "timeoutSeconds": 30,
    "fallbackThreshold": 0.2,
    "fallbackWindowSeconds": 300,
    "recoveryCooldownSeconds": 300,
    "healthCheckIntervalSeconds": 60,
    "recoveryStrategy": "consecutive",
    "probeUrl": "www.github.com:443",
    "probeTimeoutSeconds": 10,
    "recoveryConsecutiveThreshold": 3
  }
}
```

## Configuration Fields

### `mode` (string, default: `"off"`)

Proxy operating mode. Possible values:
- `"off"`: Proxy disabled, use direct connection
- `"http"`: HTTP/HTTPS proxy using CONNECT method
- `"socks5"`: SOCKS5 proxy
- `"system"`: Automatically detect and use system proxy settings

### `url` (string, optional)

Proxy server URL. Required when `mode` is `"http"` or `"socks5"`.

Examples:
- `"http://proxy.example.com:8080"`
- `"https://secure-proxy.example.com:8443"`
- `"socks5://127.0.0.1:1080"`

**Note**: The URL may contain credentials in the format `http://user:pass@proxy.example.com:8080`, but using separate `username` and `password` fields is recommended for clarity.

### `username` (string, optional)

Username for proxy authentication. Used for Basic Auth with HTTP proxies or username/password authentication with SOCKS5 proxies.

### `password` (string, optional)

Password for proxy authentication.

**Security Note**: Currently stored in plain text. Secure credential storage will be added in P6.

### `disableCustomTransport` (boolean, default: `false`)

When set to `true` and proxy is enabled, the application will:
1. **Disable custom transport layer**: Use libgit2's default HTTP transport instead of the custom subtransport
2. **Disable Fake SNI**: Always use real SNI to avoid potential conflicts and fingerprinting issues
3. **Disable IP optimization**: Skip IP pool and custom TLS features

**Important**: When `proxy.mode` is not `"off"`, this flag is **automatically forced to `true`** regardless of the user-configured value. This is a design choice to reduce complexity and avoid potential issues when using proxies.

**Use Cases**:
- Simplify debugging by using standard transport
- Avoid compatibility issues with certain proxy servers
- Reduce fingerprinting risks in proxy environments

**Trade-offs**:
- Loses benefits of Fake SNI (bypass certain SNI-based blocking)
- Loses benefits of IP optimization (automatic IP selection)
- Loses adaptive TLS metrics and auto-disable features

### `timeoutSeconds` (number, default: `30`)

Connection timeout in seconds for proxy connections.

### `fallbackThreshold` (number, default: `0.2`)

Failure rate threshold (0.0-1.0) to trigger automatic fallback to direct connection. Default is 0.2 (20% failure rate).

**✅ P5.4 Implemented**: Automatic fallback is now fully functional.

- Validates input: `window_seconds` must be > 0 (falls back to 60 if 0), `threshold` is clamped to [0.0, 1.0]
- Uses sliding window failure detection to calculate real-time failure rates
- Automatically triggers fallback when failure rate exceeds threshold
- Prevents duplicate fallback triggers with state management

**Tuning Tips**:
- **Lower threshold (0.1-0.15)**: More aggressive fallback, better for strict availability requirements
- **Higher threshold (0.3-0.5)**: More tolerant, better for unstable networks with intermittent failures
- **Default (0.2)**: Balanced approach, suitable for most scenarios

### `fallbackWindowSeconds` (number, default: `300`)

Time window in seconds for calculating failure rate. Default is 300 seconds (5 minutes).

**✅ P5.4 Implemented**: Sliding window calculation is active.

- Window is validated: must be > 0 (falls back to 60 seconds if invalid)
- Older connection attempts outside the window are automatically pruned
- Only attempts within the window contribute to failure rate calculation

**Tuning Tips**:
- **Shorter window (60-120s)**: Faster reaction to proxy issues, more sensitive to bursts
- **Longer window (600-900s)**: Smoother failure rate calculation, less prone to false positives
- **Default (300s = 5min)**: Good balance between responsiveness and stability

### `recoveryCooldownSeconds` (number, default: `300`)

Cooldown period in seconds before attempting to recover proxy after fallback. Default is 300 seconds (5 minutes).

**Note**: This feature will be implemented in P5.5.

### `healthCheckIntervalSeconds` (number, default: `60`)

Interval in seconds between proxy health checks during recovery. Default is 60 seconds (1 minute).

**Note**: This feature will be implemented in P5.5.

### `recoveryStrategy` (string, default: `"consecutive"`)

Strategy for determining when to recover proxy after fallback. Possible values:
- `"immediate"`: Recover after one successful health check
- `"consecutive"`: Recover after N consecutive successful health checks (configurable via `recoveryConsecutiveThreshold`)
- `"exponential-backoff"`: Use exponential backoff strategy (future implementation)

**Note**: This feature is implemented in P5.5.

### `probeUrl` (string, default: `"www.github.com:443"`)

Target host:port for health check probes during recovery. The health checker attempts TCP connections to this target to verify proxy connectivity.

**Format**: `"host:port"` (e.g., `"www.github.com:443"`, `"example.com:80"`)

**Validation**:
- Must contain a colon separating host and port
- Port must be a valid number (1-65535)

**Recommendations**:
- Use a reliable, publicly accessible endpoint
- Choose an endpoint relevant to your proxy's primary use case (e.g., GitHub endpoints if proxy is mainly used for git operations)

**Note**: This feature is implemented in P5.5.

### `probeTimeoutSeconds` (number, default: `10`)

Timeout in seconds for each health check probe. If a probe doesn't connect within this time, it's considered a failure.

**Valid Range**: 1-60 seconds

**Validation**:
- Must be at least 1 second
- Must not exceed 60 seconds
- Warning if close to `timeoutSeconds` (within 80%)

**Recommendations**:
- **Fast networks**: 5-10 seconds
- **Slow/congested networks**: 15-30 seconds
- Should be significantly shorter than `timeoutSeconds` to provide useful feedback

**Note**: This feature is implemented in P5.5.

### `recoveryConsecutiveThreshold` (number, default: `3`)

Number of consecutive successful health checks required before recovering from fallback (when using `"consecutive"` recovery strategy).

**Valid Range**: 1-10

**Validation**:
- Must be at least 1
- Must not exceed 10
- Warning if set to 1 with `"consecutive"` strategy (consider `"immediate"` instead)

**Recommendations**:
- **Reliable proxies**: 1-2 (faster recovery)
- **Unstable proxies**: 3-5 (more conservative)
- **Critical operations**: 5-10 (maximum confidence before recovery)

**Note**: This feature is implemented in P5.5.

### `debugProxyLogging` (boolean, default: `false`)

Enable debug-level proxy logging for detailed connection information. When enabled, outputs sanitized proxy URLs, authentication status, connection timing, and custom transport layer status at debug log level.

**Recommendations**:
- Enable for troubleshooting proxy connection issues
- Disable in production to reduce log volume
- Credentials are always sanitized even when enabled

**Log Output Includes**:
- Proxy URL (sanitized, no credentials)
- Proxy type (HTTP/SOCKS5)
- Authentication status (present/absent, not actual credentials)
- Connection establishment timing
- Custom transport layer status
- Failure reasons and retry attempts

**Note**: This feature is implemented in P5.6.

## Configuration Examples

### Example 1: Disabled Proxy (Default)

```json
{
  "proxy": {
    "mode": "off"
  }
}
```

All other fields will use default values when not specified.

### Example 2: HTTP Proxy Without Authentication

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.company.com:8080"
  }
}
```

### Example 3: HTTP Proxy With Authentication

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.company.com:8080",
    "username": "myuser",
    "password": "mypassword"
  }
}
```

### Example 4: SOCKS5 Proxy Without Authentication

```json
{
  "proxy": {
    "mode": "socks5",
    "url": "socks5://127.0.0.1:1080"
  }
}
```

**Note**: SOCKS5 proxy is fully implemented in P5.2, supporting:
- No Auth (0x00) method
- Username/Password Auth (0x02) method
- IPv4, IPv6, and domain name address types
- Comprehensive timeout control and error handling

### Example 4b: SOCKS5 Proxy With Authentication

```json
{
  "proxy": {
    "mode": "socks5",
    "url": "socks5://proxy.example.com:1080",
    "username": "socksuser",
    "password": "sockspass"
  }
}
```

When username and password are provided, the connector will use Username/Password authentication (method 0x02).

### Example 5: System Proxy Auto-Detection

```json
{
  "proxy": {
    "mode": "system"
  }
}
```

The application will automatically detect proxy settings from:
- **Windows**: Internet Settings registry (`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Internet Settings`)
- **macOS**: Network preferences via `scutil --proxy`
- **Linux**: Environment variables (`http_proxy`, `https_proxy`, `all_proxy`)

**Detected Proxy Types**: The system can detect both HTTP and SOCKS5 proxies. The proxy type is determined from the URL scheme or system settings.

### Example 6: Custom Timeout and Fallback Settings

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.example.com:8080",
    "timeoutSeconds": 60,
    "fallbackThreshold": 0.3,
    "fallbackWindowSeconds": 600
  }
}
```

### Example 7: Aggressive Fallback for High Availability (P5.4)

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.company.com:8080",
    "username": "user",
    "password": "pass",
    "fallbackThreshold": 0.1,
    "fallbackWindowSeconds": 120
  }
}
```

**Use Case**: Critical operations where availability is paramount.

**Behavior**:
- Fallback triggers at 10% failure rate
- Calculates failure rate over 2-minute window
- Quickly switches to direct connection on proxy issues
- Best for: CI/CD pipelines, automated scripts

**Trade-offs**:
- May fallback too early during brief network hiccups
- Less tolerant of temporary proxy instability

### Example 8: Tolerant Fallback for Unstable Networks (P5.4)

```json
{
  "proxy": {
    "mode": "socks5",
    "url": "socks5://proxy.example.com:1080",
    "fallbackThreshold": 0.4,
    "fallbackWindowSeconds": 900
  }
}
```

**Use Case**: Environments with intermittent connectivity or proxy instability.

**Behavior**:
- Fallback triggers at 40% failure rate
- Calculates failure rate over 15-minute window
- More tolerant of temporary failures
- Best for: Mobile networks, developing regions, unstable corporate proxies

**Trade-offs**:
- Slower to react to persistent proxy failures
- Users may experience more failed requests before fallback

### Example 9: Fast Recovery for Reliable Proxies (P5.5)

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.company.com:8080",
    "username": "user",
    "password": "pass",
    "recoveryStrategy": "immediate",
    "recoveryCooldownSeconds": 60,
    "healthCheckIntervalSeconds": 30,
    "probeUrl": "www.github.com:443",
    "probeTimeoutSeconds": 5,
    "recoveryConsecutiveThreshold": 1
  }
}
```

**Use Case**: Stable enterprise proxies with occasional maintenance windows.

**Behavior**:
- Single successful health check triggers recovery (`recoveryConsecutiveThreshold`: 1)
- Short cooldown (1 minute) after fallback
- Frequent health checks (every 30 seconds)
- Fast probe timeout (5 seconds) for quick feedback
- Best for: Reliable corporate proxies, managed infrastructure

**Trade-offs**:
- May attempt recovery too quickly after transient failures
- More health check overhead (higher probe frequency)

### Example 10: Conservative Recovery for Unstable Proxies (P5.5)

```json
{
  "proxy": {
    "mode": "socks5",
    "url": "socks5://proxy.example.com:1080",
    "recoveryStrategy": "consecutive",
    "recoveryCooldownSeconds": 600,
    "healthCheckIntervalSeconds": 120,
    "probeUrl": "www.google.com:443",
    "probeTimeoutSeconds": 20,
    "recoveryConsecutiveThreshold": 5
  }
}
```

**Use Case**: Unreliable proxies that may fail intermittently.

**Behavior**:
- Requires 5 consecutive successful health checks (`recoveryConsecutiveThreshold`: 5)
- Long cooldown (10 minutes) after fallback
- Infrequent health checks (every 2 minutes)
- Generous probe timeout (20 seconds) for slow connections
- Best for: Unstable proxies, shared/overloaded infrastructure

**Trade-offs**:
- Slower to recover even when proxy is stable
- May stay in direct connection mode longer than necessary

### Example 11: Balanced Fallback and Recovery (P5.5)

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.company.com:8080",
    "fallbackThreshold": 0.2,
    "fallbackWindowSeconds": 300,
    "recoveryStrategy": "consecutive",
    "recoveryCooldownSeconds": 300,
    "healthCheckIntervalSeconds": 60,
    "probeUrl": "www.github.com:443",
    "probeTimeoutSeconds": 10,
    "recoveryConsecutiveThreshold": 3
  }
}
```

**Use Case**: Most common production scenarios with balanced reliability.

**Behavior**:
- Default fallback: 20% failure rate over 5 minutes
- Default recovery: 3 consecutive successes (`recoveryConsecutiveThreshold`: 3)
- Default cooldown: 5 minutes
- Default health checks: every 60 seconds
- Standard probe timeout: 10 seconds
- Best for: General production use, mixed network conditions

**Recommendation**: Start with these defaults and adjust based on observed behavior.

### Example 12: Manual Control with Disabled Auto-Recovery (P5.5)

```json
{
  "proxy": {
    "mode": "http",
    "url": "http://proxy.company.com:8080",
    "fallbackThreshold": 0.2,
    "fallbackWindowSeconds": 300,
    "recoveryStrategy": "consecutive",
    "recoveryCooldownSeconds": 3600,
    "healthCheckIntervalSeconds": 300
  }
}
```

**Use Case**: Environments requiring manual intervention for recovery.

**Behavior**:
- Automatic fallback still enabled (20% threshold)
- Very long cooldown (1 hour) effectively disables auto-recovery
- Infrequent health checks (every 5 minutes) for monitoring only
- Requires manual recovery via UI or API
- Best for: Strict change control environments, manual approval workflows

**Note**: Set `recoveryCooldownSeconds` to a very high value (e.g., 86400 for 24 hours) to effectively disable automatic recovery while maintaining manual recovery capability.

## Troubleshooting Automatic Fallback (P5.4)

### Scenario: Fallback Not Triggering

**Symptoms**: Proxy keeps failing but doesn't fallback to direct connection.

**Possible Causes**:
1. **Failure rate below threshold**
   - Check: `fallbackThreshold` is too high (e.g., 0.9)
   - Solution: Lower threshold to 0.2-0.3

2. **Window too large**
   - Check: `fallbackWindowSeconds` is very large (> 1800)
   - Solution: Reduce window to 300-600 seconds

3. **Not enough attempts in window**
   - Check: Very few connection attempts within window
   - Solution: Use shorter window (60-120s) or wait for more attempts

**Debug Steps**:
```
1. Check logs for "Proxy connection failure recorded" messages
2. Look for "Failure detector updated" debug logs showing failure rate
3. Verify threshold is crossed: failure_rate >= threshold
4. Confirm state transitions to "fallback"
```

### Scenario: Fallback Triggering Too Often

**Symptoms**: Proxy fallbacks to direct connection even with minor issues.

**Possible Causes**:
1. **Threshold too low**
   - Check: `fallbackThreshold` is very low (< 0.1)
   - Solution: Increase threshold to 0.2-0.3

2. **Window too small**
   - Check: `fallbackWindowSeconds` is very small (< 60)
   - Solution: Increase window to 300-600 seconds

3. **Burst failures triggering fallback**
   - Check: Short burst of failures within small window
   - Solution: Use longer window to smooth out bursts

**Debug Steps**:
```
1. Check logs for "Automatic proxy fallback triggered" messages
2. Review failure_rate, threshold, and window_seconds in log
3. Analyze failure patterns: burst vs sustained failures
4. Adjust threshold/window based on patterns
```

### Scenario: Configuration Not Taking Effect

**Symptoms**: Changes to `fallbackThreshold` or `fallbackWindowSeconds` seem ignored.

**Solutions**:
1. **Restart application**: Configuration is loaded at startup
2. **Verify JSON syntax**: Ensure no syntax errors in `config.json`
3. **Check for typos**: Field names are case-sensitive (`fallbackThreshold` not `fallbackthreshold`)
4. **Look for validation warnings**: Check logs for configuration validation messages

**Example Log Messages**:
```
WARN Invalid window_seconds=0, using default 60 seconds
WARN Threshold 1.5 out of range [0.0, 1.0], clamped to 1.0
```

## System Proxy Detection

### Detection Priority

When `mode` is set to `"system"`, the application follows this detection order:

1. **Platform-specific detection**:
   - Windows: Registry settings
   - macOS: `scutil --proxy` command
   - Linux: Skip to step 2

2. **Environment variables** (all platforms):
   - `HTTPS_PROXY` or `https_proxy`
   - `HTTP_PROXY` or `http_proxy`
   - `ALL_PROXY` or `all_proxy`

3. **No proxy detected**: Falls back to direct connection

### Compatibility Notes

- **Windows**: Requires access to registry. May not work in sandboxed environments.
- **macOS**: Requires `scutil` command to be available.
- **Linux**: Relies on environment variables set by the user or system.
- **PAC files**: Not currently supported. Only static proxy configurations are detected.

## Mutual Exclusion with Other Features

### Fake SNI

When proxy is enabled (`mode` is not `"off"`), Fake SNI is **automatically disabled** to:
- Avoid potential conflicts between proxy and SNI manipulation
- Reduce fingerprinting risks (proxy + Fake SNI may create unique signatures)
- Simplify the transport layer logic

This is enforced in code and cannot be overridden by configuration.

### IP Pool and Custom Transport

When proxy is enabled, the `disableCustomTransport` flag is **automatically set to `true`**, which:
- Disables the custom HTTPS subtransport layer
- Uses libgit2's default HTTP transport
- Disables IP pool and IP optimization features

This ensures compatibility with proxy servers and reduces complexity.

### Verification

To verify that Fake SNI and custom transport are disabled when using proxy:
1. Check application logs for messages like:
   - `"Proxy enabled, force disable custom transport and Fake SNI"`
   - `"Custom transport disabled, using libgit2 default HTTP"`
2. Observe that timing events do not include `used_fake_sni` or custom transport metrics
3. Verify that proxy URL appears in connection logs

## Hot Reload

The proxy configuration supports hot reloading:
1. Modify `config.json` and save
2. The application will detect the change and reload the configuration
3. New tasks will use the updated proxy settings
4. Existing tasks continue with their original settings

**Note**: Changing proxy settings may affect ongoing operations. It's recommended to apply changes when no critical tasks are running.

## Troubleshooting

### Proxy connection fails

1. Verify the proxy URL format is correct:
   - HTTP: `http://host:port` or `https://host:port`
   - SOCKS5: `socks5://host:port` or `socks://host:port`
2. Check username/password if authentication is required
3. Ensure the proxy server is accessible from your network
4. Increase `timeoutSeconds` if the network is slow
5. Check application logs for detailed error messages:
   - `ProxyError::Network`: Connection or DNS issues
   - `ProxyError::Auth`: Authentication failure (407 for HTTP, auth failure for SOCKS5)
   - `ProxyError::Proxy`: Proxy server errors or protocol issues
   - `ProxyError::Timeout`: Connection timeout

### SOCKS5-specific issues

1. **Authentication method not supported**: Ensure your SOCKS5 server supports No Auth (0x00) or Username/Password (0x02) methods. GSSAPI and other methods are not yet supported.
2. **Version mismatch**: The connector requires SOCKS5 (version 0x05). SOCKS4 is not supported.
3. **Address type not supported**: Verify the target host can be resolved. The connector supports IPv4, IPv6, and domain names.
4. **REP error codes**: Check logs for specific SOCKS5 response codes:
   - `0x01`: General SOCKS server failure
   - `0x02`: Connection not allowed by ruleset
   - `0x03`: Network unreachable
   - `0x04`: Host unreachable
   - `0x05`: Connection refused
   - `0x06`: TTL expired
   - `0x07`: Command not supported
   - `0x08`: Address type not supported

### System proxy detection not working

1. Verify system proxy is actually configured in OS settings
2. On Linux, ensure environment variables are set
3. Check application logs for detection results
4. Try manual configuration with explicit `url` instead of `"system"` mode

### Tasks fail after enabling proxy

1. Verify proxy server supports CONNECT method (for HTTP) or SOCKS5 protocol
2. Check if proxy requires authentication and credentials are correct
3. Try setting `disableCustomTransport: true` explicitly (though it should be automatic)
4. Disable proxy temporarily to verify the issue is proxy-related

### Logs show sanitized proxy URL

This is by design to prevent credential leakage. Full URLs with credentials are only logged at debug level when explicitly enabled.

## Security Considerations

1. **Credential Storage**: Currently stored in plain text in `config.json`. Keep this file secure with appropriate file system permissions.

2. **Credential Logging**: Credentials are sanitized in logs by default. Full URLs are only logged at debug level.

3. **Man-in-the-Middle**: When using HTTP (not HTTPS) proxy, traffic between your application and the proxy is unencrypted. Use HTTPS proxies when possible.

4. **Trust**: Ensure you trust the proxy server, as it can see all your Git traffic.

## P5.6 Frontend UI Usage (NEW)

### Proxy Configuration Panel

The proxy configuration UI is integrated into the main configuration view and provides an intuitive interface for all proxy settings.

#### Accessing Proxy Settings

1. Open the application
2. Navigate to the configuration/settings page
3. Find the "Proxy Configuration" section

#### Configuration Steps

**Method 1: Manual Configuration**

1. Select proxy mode from dropdown:
   - **Off**: Disable proxy (direct connection)
   - **HTTP/HTTPS**: Use HTTP CONNECT tunnel
   - **SOCKS5**: Use SOCKS5 protocol
   - **System**: Auto-detect from system settings

2. Enter proxy server address:
   - Format: `http://proxy.example.com:8080` or `socks5://127.0.0.1:1080`
   - For System mode, this field is auto-filled after detection

3. (Optional) Enter authentication credentials:
   - Username
   - Password
   - Leave blank if proxy doesn't require authentication

4. Review advanced settings:
   - **Disable Custom Transport**: Automatically enabled when proxy is active
   - **Enable Proxy Debug Logging**: Show detailed connection information in logs

5. Click "Save Configuration" to apply changes

**Method 2: System Proxy Detection**

1. Select "System" mode from the dropdown
2. Click "Detect System Proxy" button
3. Wait for detection result:
   - If detected: Shows proxy URL and type
   - If not detected: Shows "No system proxy detected"
4. Click "One-Click Apply" to auto-fill the configuration
5. Adjust mode if needed (e.g., change from auto-detected HTTP to SOCKS5)
6. Click "Save Configuration"

#### Proxy Status Panel

The status panel provides real-time information about proxy operation:

**Status Indicators**:
- **Current Mode**: Shows Off/HTTP/SOCKS5/System
- **Running State**: 
  - 🟢 Enabled: Proxy is active and working
  - ⚪ Disabled: Proxy is turned off
  - 🟠 Fallback: Auto-downgraded to direct connection due to failures
  - 🔵 Recovering: Testing proxy availability for recovery
- **Proxy Server**: Displays configured proxy address (credentials hidden)
- **Custom Transport Layer**: Shows if custom transport is enabled/disabled

**Fallback Information** (shown when in Fallback state):
- Reason for fallback
- Number of consecutive failures
- Automatic recovery status

**Recovery Information** (shown when in Recovering state):
- Current recovery progress
- Next health check countdown

**Health Check Stats**:
- Success rate progress bar with color coding:
  - Green: ≥80% success rate
  - Orange: 50-80% success rate
  - Red: <50% success rate

#### Manual Control

**Force Fallback** (available when proxy is enabled):
- Click "Force Fallback" button to manually switch to direct connection
- Useful for troubleshooting or bypassing proxy temporarily
- Can be reversed with "Force Recovery"

**Force Recovery** (available in Fallback/Recovering state):
- Click "Force Recovery" button to immediately re-enable proxy
- Bypasses cooldown period and recovery threshold
- Use when you've confirmed proxy is available again

### Debug Logging

To enable detailed proxy connection logs:

1. In Proxy Configuration panel, check "Enable Proxy Debug Logging"
2. Save configuration
3. View logs in the application's log viewer or log files
4. Debug logs include (with credentials sanitized):
   - Proxy URL and type
   - Connection establishment steps
   - Authentication status
   - Connection timing
   - Custom transport layer status
   - Failure reasons

**Log Levels**:
- **Info**: Basic proxy state changes
- **Debug**: Detailed connection information (only when `debugProxyLogging` is enabled)
- **Warn**: Connection failures, fallback triggers
- **Error**: Critical proxy errors

### System Proxy Compatibility

**Windows**:
- Reads from Registry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`
- Supports HTTP and SOCKS5 detection
- Requires proxy to be enabled in "Internet Options"

**macOS**:
- Reads from `scutil --proxy` command output
- Supports HTTP, HTTPS, and SOCKS5 detection
- Requires proxy configuration in "System Preferences > Network"

**Linux**:
- Reads from environment variables: `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`
- Case-insensitive detection
- Supports all proxy types

### Troubleshooting with UI

1. **Proxy not connecting**:
   - Check status panel for error messages
   - Enable debug logging to see detailed connection attempts
   - Try "Force Fallback" then "Force Recovery" to reset connection

2. **Frequent fallbacks**:
   - Check health check success rate in status panel
   - Review fallback reason message
   - Consider adjusting `fallbackThreshold` in config.json if needed

3. **System detection fails**:
   - Status will show "No system proxy detected"
   - Verify proxy is configured in OS settings
   - Try manual configuration as fallback

4. **Credentials not working**:
   - Double-check username/password (shown as masked in UI)
   - Enable debug logging to verify auth headers are sent
   - Confirm proxy supports Basic Authentication

## Future Enhancements (P5.3-P5.7)

The following features are planned for future releases:

- ✅ **P5.1**: Full HTTP/HTTPS proxy implementation with CONNECT tunnel support (Completed)
- ✅ **P5.2**: SOCKS5 proxy protocol implementation (Completed)
- ✅ **P5.3**: System proxy auto-detection (Windows, macOS, Linux) (Completed)
- ✅ **P5.4**: Automatic fallback to direct connection with failure detection (Completed)
  - Sliding window failure rate calculation
  - Configurable threshold and window
  - Comprehensive edge case handling and logging
  - 234 unit tests with 100% pass rate
- ✅ **P5.5**: Automatic recovery with health check probing (Completed)
- ✅ **P5.6**: Frontend UI for proxy configuration and status display (Completed)
  - ProxyConfig.vue component with system proxy detection
  - ProxyStatusPanel.vue for real-time status monitoring
  - Tauri commands: detect_system_proxy, force_proxy_fallback, force_proxy_recovery
  - Extended events with all required fields
  - Debug logging support
  - 10 new integration tests
- ⏳ **P5.7**: Comprehensive testing and production readiness validation

**Note**: P5.3 transport layer integration is complete. Proxy support is fully functional in Git operations.

## References

- Technical Design: `new-doc/TECH_DESIGN_P5_PLAN.md`
- Implementation Details: `src-tauri/src/core/proxy/`
- Configuration Loader: `src-tauri/src/core/config/loader.rs`
