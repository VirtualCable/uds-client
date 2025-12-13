# Ensure udsclient-builder is up up date
# To be executed on building/windows/ directory
docker build -t udsclient-builder .
# Get full path of the ../.. directory (i.e., the root of the project)
$projectDir = Convert-Path ../..

# Run the container with the current directory mounted
docker run --rm -v ${projectDir}:c:\crate -w /crate udsclient-builder cargo build --release
# Note: the target/release/launcher.exe binary will be created