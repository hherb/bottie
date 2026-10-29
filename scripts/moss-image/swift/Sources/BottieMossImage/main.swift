/// Offline, one-request MLX worker. Model paths are native argv; prompts and PNGs use private stdio.
import CoreGraphics
import Foundation
import ImageIO
import Metal
import MLX
import MossImage
import UniformTypeIdentifiers

let maxPromptBytes = 32 * 1024
let maxRequestBytes = maxPromptBytes * 2 + 4096 // JSON escaping plus fixed metadata allowance.
let maxPNGBytes = 25 * 1024 * 1024
let inferenceSteps = 50

/// Only protocol events may be written to stdout.
func emit(_ event: [String: Any]) throws {
    var bytes = try JSONSerialization.data(withJSONObject: event, options: [.sortedKeys])
    bytes.append(10)
    try FileHandle.standardOutput.write(contentsOf: bytes)
}
/// Parse a fixed native option without interpreting a shell or accepting process arguments from the WebView.
func option(_ name: String) throws -> String {
    let arguments = CommandLine.arguments
    guard let index = arguments.firstIndex(of: name), index + 1 < arguments.count else {
        throw MossImageError.invalidInput("missing native option")
    }
    return arguments[index + 1]
}
/// Bound fragmented stdin before decoding any JSON or loading GPU weights.
func readRequest() throws -> Request {
    var data = Data()
    while !data.contains(10) {
        guard let part = try FileHandle.standardInput.read(upToCount: 8192), !part.isEmpty else {
            throw MossImageError.invalidInput("incomplete request")
        }
        data.append(part)
        guard data.count <= maxRequestBytes else { throw MossImageError.invalidInput("request too large") }
    }
    return try JSONDecoder().decode(Request.self, from: data)
}
/// The Rust contract is independently checked at the native worker boundary.
struct Request: Decodable {
    let prompt: String
    let width: Int
    let height: Int
    let seed: UInt64
    let steps: Int
    func validate() throws {
        guard !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              prompt.utf8.count <= maxPromptBytes, width >= 256, height >= 256,
              width <= 4096, height <= 4096, width % 16 == 0, height % 16 == 0,
              width * height <= 1456 * 1456, seed <= UInt64(Int64.max), steps == inferenceSteps,
              !prompt.unicodeScalars.contains(where: { $0.properties.generalCategory == .control
                  && $0 != "\n" && $0 != "\t" }) else {
            throw MossImageError.invalidInput("request outside bounds")
        }
    }
}
/// Encode pixels in memory; Rust owns normalization and durable filenames.
func encodePNG(_ rgb: (pixels: [UInt8], width: Int, height: Int)) throws -> Data {
    let bytes = Data(rgb.pixels) as CFData
    let output = NSMutableData()
    guard let provider = CGDataProvider(data: bytes),
          let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
          let image = CGImage(width: rgb.width, height: rgb.height, bitsPerComponent: 8,
              bitsPerPixel: 24, bytesPerRow: rgb.width * 3, space: colorSpace,
              bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.none.rawValue), provider: provider,
              decode: nil, shouldInterpolate: false, intent: .defaultIntent),
          let destination = CGImageDestinationCreateWithData(output, UTType.png.identifier as CFString, 1, nil)
    else { throw MossImageError.invalidInput("PNG codec unavailable") }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination), output.length <= maxPNGBytes else {
        throw MossImageError.invalidInput("PNG output exceeds bounds")
    }
    return output as Data
}
/// Terminate this owned inference process if Bottie exits without running its normal cancellation path.
func parentWatchdog() -> DispatchSourceTimer {
    let parent = getppid()
    let timer = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
    timer.schedule(deadline: .now() + 1, repeating: 1)
    timer.setEventHandler { if getppid() != parent { _exit(1) } }
    timer.resume()
    return timer
}
/// Use only local snapshots; the upstream tokenizer API receives a folder, never a Hub identifier.
func main() async throws {
    let watchdog = parentWatchdog()
    defer { watchdog.cancel() }
    guard MTLCreateSystemDefaultDevice() != nil else { throw MossImageError.invalidInput("Metal required") }
    if CommandLine.arguments.contains("--probe") {
        let values = Device.withDefaultDevice(.gpu) {
            let input = MLXArray([Float(1), Float(2)])
            let output = input + input
            eval(output)
            return output.asArray(Float.self)
        }
        guard values == [2, 4] else { throw MossImageError.invalidInput("Metal probe failed") }
        try emit(["event": "probe", "runtime": "mlx", "metal": true])
        return
    }
    let directory = try option("--model-directory")
    let model = try option("--model-id")
    let revision = try option("--revision")
    let snapshot = URL(fileURLWithPath: directory, isDirectory: true)
    try emit(["event": "ready", "protocolVersion": 1, "modelId": model,
              "revision": revision, "runtime": "mlx"])
    let request = try readRequest()
    try request.validate()
    let tokenizer = try await MossImagePromptEncoder.loadTokenizer(snapshot: snapshot)
    let tokens = tokenizer.encode(text: MossImagePromptEncoder.render(request.prompt), addSpecialTokens: false)
    guard tokens.count <= MossImagePromptEncoder.maxSequenceLength else {
        throw MossImageError.invalidInput("layout exceeds encoder token limit")
    }
    let png = try await Device.withDefaultDevice(.gpu) {
        let pipeline = try await MossImagePipeline.load(transformerSnapshot: snapshot, dtype: .bfloat16)
        var parameters = MossImageGenerationParameters()
        parameters.width = request.width
        parameters.height = request.height
        parameters.seed = request.seed
        parameters.steps = inferenceSteps
        let image = try pipeline.generate(prompt: request.prompt, parameters: parameters, releaseTextEncoder: true,
            onStep: { step, _ , _ in try? emit(["event": "progress", "step": step + 1]) })
        return try encodePNG(MossImagePipeline.toRGB8(image))
    }
    try emit(["event": "complete", "modelId": model, "revision": revision, "width": request.width,
              "height": request.height, "seed": request.seed, "png": png.base64EncodedString()])
}
do { try await main() }
catch { try? emit(["event": "error"]); exit(1) }
