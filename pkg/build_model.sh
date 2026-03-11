# model pkg command, run when model changes, or need a local copy for final pkg
MODELS_VERSION=1.0
pkgbuild --root pkgroot_models --identifier com.tilesprivacy.tiles_models --version "$MODELS_VERSION" tiles-model.pkg
