# Loopback TLS fixture

`localhost.pem` and `localhost.p12` are a self-signed certificate and public
fixture-only private key for the acquisition transport downgrade regression.
The PKCS12 password is `fixture`. Never use this identity outside tests.
The certificate is trusted only by that test's reqwest client; no system trust
store is changed. The server binds a disposable literal loopback port.
