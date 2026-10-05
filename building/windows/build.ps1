# Ensure udslauncher-builder is up up date
# To be executed on building/windows/ directory
docker build -t udslauncher-builder .
# Get full path of the ../.. directory (i.e., the root of the project)
$projectDir = Convert-Path ../..

# Features requested through UDS_CARGO_FEATURES (comma separated)
$cargoCmd = @("cargo", "build", "--release")
if ($env:UDS_CARGO_FEATURES) { $cargoCmd += @("--features", $env:UDS_CARGO_FEATURES) }

# Run the container with the current directory mounted
docker run --rm -v ${projectDir}:c:\crate -w /crate udslauncher-builder @cargoCmd
# Note: the target/release/launcher.exe binary will be created