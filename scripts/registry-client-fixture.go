//go:build ignore

// Synthetic local test keys and a short-lived loopback TLS certificate. Never production provisioning.
package main

import (
	"crypto/ed25519"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"time"
)

func main() {
	if len(os.Args) != 2 {
		panic("fixture output directory required")
	}
	root := os.Args[1]
	if err := os.MkdirAll(root, 0700); err != nil {
		panic("fixture directory failed")
	}
	write := func(name string, data []byte) {
		f, err := os.OpenFile(filepath.Join(root, name), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
		if err != nil {
			panic("fixture output already exists or cannot be created")
		}
		if _, err = f.Write(data); err != nil {
			f.Close()
			panic("fixture write failed")
		}
		if f.Close() != nil {
			panic("fixture close failed")
		}
	}
	pub, key, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		panic("fixture signing key failed")
	}
	private, err := x509.MarshalPKCS8PrivateKey(key)
	if err != nil {
		panic("fixture signing encoding failed")
	}
	write("signing.pem", pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: private}))
	trust := map[string]any{"schema": "glyphshift.release-trust/1", "issuer": "urn:glyphshift:registry:fixture", "keys": []any{map[string]string{"id": "fixture-key", "publicKey": base64.RawURLEncoding.EncodeToString(pub), "state": "active"}}}
	encoded, err := json.Marshal(trust)
	if err != nil {
		panic("fixture trust failed")
	}
	write("trust.json", encoded)
	tlsKey, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		panic("fixture TLS key failed")
	}
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 120))
	if err != nil {
		panic("fixture serial failed")
	}
	template := &x509.Certificate{SerialNumber: serial, Subject: pkix.Name{CommonName: "Glyphshift synthetic loopback fixture"}, NotBefore: time.Now().Add(-time.Hour), NotAfter: time.Now().Add(4 * time.Hour),
		KeyUsage: x509.KeyUsageDigitalSignature | x509.KeyUsageKeyEncipherment | x509.KeyUsageCertSign, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth}, BasicConstraintsValid: true, IsCA: true,
		DNSNames: []string{"localhost"}, IPAddresses: []net.IP{net.ParseIP("127.0.0.1")}}
	cert, err := x509.CreateCertificate(rand.Reader, template, template, &tlsKey.PublicKey, tlsKey)
	if err != nil {
		panic("fixture TLS certificate failed")
	}
	write("tls-cert.pem", pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: cert}))
	write("tls-key.pem", pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(tlsKey)}))
}
