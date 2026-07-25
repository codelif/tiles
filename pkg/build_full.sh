#!/usr/bin/env bash

set -euo pipefail

VERSION=$(grep '^version' tiles/Cargo.toml | head -1 | awk -F'"' '{print $2}')


productbuild \
  --distribution pkg/distribution.xml \
  --resources pkg/resources \
  --package-path pkg/  \
  pkg/tiles-full-unsigned.pkg


# signing
# 
productsign \
  --sign "$DEVELOPER_ID_INSTALLER" \
  pkg/tiles-full-unsigned.pkg \
  pkg/tiles-full.pkg

# # notarizing
# # 
xcrun notarytool submit pkg/tiles-full.pkg \
  --keychain-profile "tiles-notary-profile" \
  --wait

# # # staple the approval ticket to pkg
xcrun stapler staple pkg/tiles-full.pkg

rm pkg/tiles-full-unsigned.pkg
