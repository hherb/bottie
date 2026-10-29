// swift-tools-version: 6.3
/// Standalone private-stdio bridge to the pinned upstream Swift/MLX MOSS pipeline.
import PackageDescription
let package = Package(
    name: "BottieMossImage", platforms: [.macOS(.v26)],
    products: [.executable(name: "BottieMossImage", targets: ["BottieMossImage"])],
    dependencies: [
        .package(url: "https://github.com/xocialize/moss-image-swift.git",
                 revision: "6c26a0296ba2d2a09e061d89a9b9b758786cd186"),
        .package(url: "https://github.com/ml-explore/mlx-swift.git", exact: "0.32.3"),
    ],
    targets: [.executableTarget(name: "BottieMossImage", dependencies: [
        .product(name: "MossImage", package: "moss-image-swift"),
        .product(name: "MLX", package: "mlx-swift"),
    ])]
)
