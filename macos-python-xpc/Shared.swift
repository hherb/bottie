import Foundation

/// Fixed private service identifier embedded inside the development bundle.
let pythonRunnerServiceIdentifier = "com.bottie.python-runner"

/// Maximum request carried through the proof transport, matching the runner's stdin limit.
let maximumRequestBytes = 256 * 1_024

/// Maximum response retained from the runner's private stdout pipe.
let maximumResponseBytes = 96 * 1_024

/// Private XPC protocol shared by the native proof and opt-in product transport.
@objc protocol BottiePythonRunnerServiceProtocol {
  /// Starts one runner process and returns its proof-only process identifier.
  func start(
    _ request: Data,
    identifier: String,
    withReply reply: @escaping (NSNumber?, String?) -> Void
  )

  /// Polls one execution without exposing service or filesystem internals.
  func poll(
    _ identifier: String,
    withReply reply: @escaping (String, Data?, String?) -> Void
  )

  /// Cancels one running execution by its caller-generated identifier.
  func cancel(_ identifier: String, withReply reply: @escaping (Bool) -> Void)

  /// Probes whether the restricted service can read one host-owned denial fixture.
  func canReadHostFixture(_ path: String, withReply reply: @escaping (Bool) -> Void)
}
