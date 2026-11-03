# Loopback TLS fixture

`localhost.pem` and `localhost.p12` are a self-signed certificate and public
fixture-only private key for the acquisition transport downgrade regression.
The PKCS12 password is `fixture`. Never use this identity outside tests.
The certificate is trusted only by that test's reqwest client; no system trust
store is changed. The server binds a disposable literal loopback port.

The certificate is valid for 100 years so the regression never expires. Apple's
TLS stack refuses server certificates valid for more than 825 days, so the test
does not run on macOS; its redirect-downgrade policy is platform-independent.
