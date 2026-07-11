# model pkg command, run when model changes, or need a local copy for final pkg
MODELS_VERSION=1.0

# pkgbuild --root pkgroot_gptoss_gguf --identifier com.tilesprivacy.tiles_models_gguf --version "$MODELS_VERSION" pkg/tiles-model_gguf.pkg

# Since pkg installer can't build with large files (here 11gb model) we
# first make a tar of it using `tar --zstd -cf gpt-oss-20b-GGUF.tar.zst huggingface/hub/models--`
# (huggingface is dir from which HF stuff starts with)

# split into multiple 2gb files using `split -b 2048m gpt-oss-20b-GGUF.tar.zst gpt-oss-20b-GGUF.tar.zst.part.`.

# and then during installation, joins each and extract it to the directory

# 

pkgbuild --root pkgroot_gptoss_gguf --scripts pkg/scripts/gguf  --identifier com.tilesprivacy.tiles_models_gguf --version "$MODELS_VERSION" pkg/tiles-model_gguf.pkg
