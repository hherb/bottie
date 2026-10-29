# Optional MOSS-Image runtime

Use `python3 install.py --download` explicitly from this checkout. Apple Silicon selects the pinned Swift/MLX
bridge; Linux/Windows require CUDA with bfloat16 support and use a dedicated Python environment. SFT is the default;
`--variant Base` selects Base. Each locked model is about 27.2 GB. Nothing runs or downloads automatically in Bottie.

See [`docs/moss-image-local.md`](../../docs/moss-image-local.md) for setup, layout prompts, bounds, qualification, and
cache reuse. Run pure setup/protocol tests with `python3 -m unittest discover -s scripts/moss-image`. Swift build files
are ignored; `swift/Package.resolved` retains the dependency graph. These workers are separate from `run_python`.
