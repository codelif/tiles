#!/usr/bin/env bash

set -euo pipefail

VERSION=$(grep '^version' tiles/Cargo.toml | head -1 | awk -F'"' '{print $2}')

# bundling the models
productbuild --package "tiles-${VERSION}".pkg --package tiles-model.pkg "tiles-${VERSION}-full-unsigned".pkg


# signing
productsign \
  --sign "$DEVELOPER_ID_INSTALLER" \
  "tiles-${VERSION}-full-unsigned.pkg" \
  "tiles-${VERSION}-full.pkg"

# notarizing
xcrun notarytool submit "tiles-${VERSION}-full.pkg"\
  --keychain-profile "tiles-notary-profile" \
  --wait

# staple the approval ticket to pkg
xcrun stapler staple "tiles-${VERSION}-full.pkg"


