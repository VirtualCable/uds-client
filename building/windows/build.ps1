# Ensure udslauncher-builder is up up date
# To be executed on building/windows/ directory
docker build -t udslauncher-builder .
# Get full path of the ../.. directory (i.e., the root of the project)
$projectDir = Convert-Path ../..

# Run the container with the current directory mounted
docker run --rm -v ${projectDir}:c:\crate -w /crate udslauncher-builder cargo build --release
# Note: the target/release/launcher.exe binary will be created