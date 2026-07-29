#!/usr/bin/env bash
# AgentStatus -- sign the app bundle with a stable, self-signed code-signing identity
# so macOS Accessibility (TCC) trust survives rebuilds/updates (decision 039).
#
# Why: an ad-hoc-signed app's Accessibility grant is keyed to its exact code hash, so
# every rebuild invalidates it -- you'd re-grant on every update, and the pip that reads
# Cursor's menu bar (decision 038) silently reads 0 until you do. Signing every build
# with the SAME self-signed cert makes TCC key on the stable signing identity instead,
# so you grant Accessibility once and it persists.
#
# Idempotent: the cert is created once and reused. Re-runnable safely. Local/dev use --
# a self-signed anchor does not help Gatekeeper for downloaded copies (those still clear
# quarantine as before), it only stabilizes the on-device identity for TCC.
#
#   hooks/sign-app.sh /Applications/AgentStatus.app
#
# Undo: security delete-identity -c "AgentStatus Self-Signed"   (removes key+cert)
set -euo pipefail

APP="${1:?usage: sign-app.sh <path-to-.app>}"
CN="AgentStatus Self-Signed"
LOGIN_KC="$(security default-keychain | tr -d ' "')"
[ -d "$APP" ] || { echo "sign-app: no such app bundle: $APP" >&2; exit 1; }

# --- ensure the self-signed code-signing identity exists ---
if ! security find-identity -p codesigning "$LOGIN_KC" | grep -qF "$CN"; then
  echo "sign-app: creating self-signed code-signing cert \"$CN\"..."
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  # Config-file form works on both macOS LibreSSL and OpenSSL (no --addext needed).
  cat > "$TMP/cert.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = v3
prompt = no
[dn]
CN = $CN
[v3]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
EOF
  openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
    -keyout "$TMP/key.pem" -out "$TMP/cert.pem" -config "$TMP/cert.cnf" >/dev/null 2>&1
  # OpenSSL 3 defaults to a PKCS12 MAC/cipher macOS's Security framework can't import;
  # -legacy (when available) emits the SHA1-MAC/3DES form macOS reads. Use a non-empty
  # transient password too -- the empty-password path is what tripped "MAC verification
  # failed" above.
  LEGACY=""
  openssl pkcs12 -export -help 2>&1 | grep -q -- "-legacy" && LEGACY="-legacy"
  P12PASS="agentstatus-transient"
  openssl pkcs12 -export $LEGACY -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
    -out "$TMP/id.p12" -passout "pass:$P12PASS" >/dev/null 2>&1
  # Import key+cert; -T lets /usr/bin/codesign use the key without a per-sign GUI prompt.
  security import "$TMP/id.p12" -k "$LOGIN_KC" -P "$P12PASS" -T /usr/bin/codesign >/dev/null
  # Trust the cert for the code-signing policy so codesign can build the chain to it.
  # User-domain (no -d/sudo); may prompt once for your login password.
  security add-trusted-cert -r trustRoot -p codeSign -k "$LOGIN_KC" "$TMP/cert.pem" >/dev/null 2>&1 || \
    echo "sign-app: note -- could not auto-trust the cert; if codesign fails, trust \"$CN\" for Code Signing in Keychain Access."
else
  echo "sign-app: reusing existing \"$CN\" identity."
fi

# --- sign the bundle (deep, force-replace the ad-hoc signature) ---
echo "sign-app: signing $APP..."
codesign --force --deep --sign "$CN" "$APP"
codesign --verify --deep "$APP" 2>&1 | sed 's/^/sign-app: /' || true
echo "sign-app: done. Designated Requirement is now stable across rebuilds:"
codesign -dr - "$APP" 2>&1 | grep -i "designated" | sed 's/^/  /' || true
