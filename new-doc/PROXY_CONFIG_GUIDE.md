# Proxy Configuration Guide (P5.0-P5.2)

## Overview

Starting from P5.0, the application supports proxy configuration for HTTP/HTTPS and SOCKS5 proxies, including automatic system proxy detection.

**Implementation Status**:
- ✅ **P5.0**: Base architecture, configuration model, state machine, system proxy detection
- ✅ **P5.1**: HTTP/HTTPS proxy with CONNECT tunnel and Basic Auth
- ✅ **P5.2**: SOCKS5 proxy with No Auth and Username/Password Auth
- ⏳ **P5.3+**: Transport layer integration, automatic fallback/recovery (in development)

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
    "recoveryStrategy": "consecutive"
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

**Note**: This feature will be implemented in P5.4. Currently, fallback is not automatic.

### `fallbackWindowSeconds` (number, default: `300`)

Time window in seconds for calculating failure rate. Default is 300 seconds (5 minutes).

### `recoveryCooldownSeconds` (number, default: `300`)

Cooldown period in seconds before attempting to recover proxy after fallback. Default is 300 seconds (5 minutes).

**Note**: This feature will be implemented in P5.5.

### `healthCheckIntervalSeconds` (number, default: `60`)

Interval in seconds between proxy health checks during recovery. Default is 60 seconds (1 minute).

**Note**: This feature will be implemented in P5.5.

### `recoveryStrategy` (string, default: `"consecutive"`)

Strategy for determining when to recover proxy after fallback. Possible values:
- `"single"`: Recover after one successful health check
- `"consecutive"`: Recover after multiple consecutive successful health checks
- `"rate"`: Recover based on success rate within a time window

**Note**: This feature will be implemented in P5.5.

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

## Future Enhancements (P5.3-P5.7)

The following features are planned for future releases:

- ✅ **P5.1**: Full HTTP/HTTPS proxy implementation with CONNECT tunnel support (Completed)
- ✅ **P5.2**: SOCKS5 proxy protocol implementation (Completed)
- ⏳ **P5.3**: Transport layer integration and mutual exclusion enforcement
- ⏳ **P5.4**: Automatic fallback to direct connection on proxy failures
- ⏳ **P5.5**: Automatic recovery with health check probing
- ⏳ **P5.6**: Frontend UI for proxy configuration and status display
- ⏳ **P5.7**: Comprehensive testing and production readiness validation

**Note**: Until P5.3 is completed, proxy configuration is available but not yet integrated with the Git transport layer. Manual testing requires actual network connections to proxy servers.

## References

- Technical Design: `new-doc/TECH_DESIGN_P5_PLAN.md`
- Implementation Details: `src-tauri/src/core/proxy/`
- Configuration Loader: `src-tauri/src/core/config/loader.rs`
