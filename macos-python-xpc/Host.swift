import Darwin
import Foundation

private let pollInterval: TimeInterval = 0.05
private let executionDeadline: TimeInterval = 40
private let cancellationDelay: TimeInterval = 0.25

/// Synchronous proof client around Bottie's private XPC protocol.
private final class ProofClient {
  private let connection: NSXPCConnection

  init() {
    connection = NSXPCConnection(serviceName: pythonRunnerServiceIdentifier)
    connection.remoteObjectInterface = NSXPCInterface(with: BottiePythonRunnerServiceProtocol.self)
    connection.resume()
  }

  deinit {
    connection.invalidate()
  }

  /// Starts one request and returns the service-owned runner process identifier.
  func start(request: Data, identifier: String) -> Int32? {
    let semaphore = DispatchSemaphore(value: 0)
    var runnerPID: Int32?
    proxy { service in
      service.start(request, identifier: identifier) { processIdentifier, _ in
        runnerPID = processIdentifier?.int32Value
        semaphore.signal()
      }
    }
    guard semaphore.wait(timeout: .now() + 5) == .success else { return nil }
    return runnerPID
  }

  /// Polls until the service returns one terminal state or the host deadline expires.
  func wait(identifier: String) -> (String, Data?, String?) {
    let deadline = Date().addingTimeInterval(executionDeadline)
    while Date() < deadline {
      let response = poll(identifier: identifier)
      if response.0 != "running" {
        return response
      }
      Thread.sleep(forTimeInterval: pollInterval)
    }
    return ("failed", nil, "proof_timeout")
  }

  /// Cancels one service-owned execution.
  func cancel(identifier: String) -> Bool {
    let semaphore = DispatchSemaphore(value: 0)
    var cancelled = false
    proxy { service in
      service.cancel(identifier) { accepted in
        cancelled = accepted
        semaphore.signal()
      }
    }
    guard semaphore.wait(timeout: .now() + 5) == .success else { return false }
    return cancelled
  }

  /// Asks the restricted service to attempt one direct host-fixture read.
  func canReadHostFixture(_ path: String) -> Bool? {
    let semaphore = DispatchSemaphore(value: 0)
    var readable: Bool?
    proxy { service in
      service.canReadHostFixture(path) { value in
        readable = value
        semaphore.signal()
      }
    }
    guard semaphore.wait(timeout: .now() + 5) == .success else { return nil }
    return readable
  }

  private func poll(identifier: String) -> (String, Data?, String?) {
    let semaphore = DispatchSemaphore(value: 0)
    var response: (String, Data?, String?) = ("failed", nil, "service_unavailable")
    proxy { service in
      service.poll(identifier) { state, result, error in
        response = (state, result, error)
        semaphore.signal()
      }
    }
    _ = semaphore.wait(timeout: .now() + 5)
    return response
  }

  private func proxy(_ operation: (BottiePythonRunnerServiceProtocol) -> Void) {
    let remote =
      connection.remoteObjectProxyWithErrorHandler { _ in } as? BottiePythonRunnerServiceProtocol
    if let remote {
      operation(remote)
    }
  }
}

/// Writes one JSON object without ever including host or bundle paths.
private func writeJSON(_ object: [String: Any]) {
  guard let data = try? JSONSerialization.data(withJSONObject: object),
    let text = String(data: data, encoding: .utf8)
  else {
    print("{\"status\":\"failed\"}")
    return
  }
  print(text)
}

/// Reads the bounded runner request from stdin so source never enters process arguments.
private func readRequest() -> Data? {
  let request = FileHandle.standardInput.readDataToEndOfFile()
  guard !request.isEmpty, request.count <= maximumRequestBytes else { return nil }
  return request
}

/// Executes the normal completion proof and returns the runner's unchanged JSON result.
private func execute(_ request: Data) -> Int32 {
  let client = ProofClient()
  let identifier = "execute-\(UUID().uuidString.lowercased())"
  guard client.start(request: request, identifier: identifier) != nil else { return 1 }
  let result = client.wait(identifier: identifier)
  guard result.0 == "completed", let data = result.1 else {
    writeJSON(["status": result.0, "error": result.2 ?? "runner_failed"])
    return 1
  }
  FileHandle.standardOutput.write(data)
  FileHandle.standardOutput.write(Data("\n".utf8))
  return 0
}

/// Starts then cancels one request, proving the service owns the child lifecycle.
private func cancel(_ request: Data) -> Int32 {
  let client = ProofClient()
  let identifier = "cancel-\(UUID().uuidString.lowercased())"
  guard client.start(request: request, identifier: identifier) != nil else { return 1 }
  Thread.sleep(forTimeInterval: cancellationDelay)
  guard client.cancel(identifier: identifier) else { return 1 }
  let result = client.wait(identifier: identifier)
  writeJSON(["status": result.0])
  return result.0 == "cancelled" ? 0 : 1
}

/// Exits without invalidating the connection after reporting the service child identifier.
private func startAndExit(_ request: Data) -> Never {
  let client = ProofClient()
  let identifier = "parent-\(UUID().uuidString.lowercased())"
  guard let processIdentifier = client.start(request: request, identifier: identifier) else {
    writeJSON(["status": "failed"])
    fflush(stdout)
    Darwin._exit(1)
  }
  writeJSON(["status": "started", "pid": processIdentifier])
  fflush(stdout)
  Darwin._exit(0)
}

/// Entrypoint for the development-only proof host embedded beside the XPC service.
@main
private struct HostMain {
  static func main() {
    let arguments = CommandLine.arguments
    guard arguments.count >= 2 else { Darwin.exit(2) }
    switch arguments[1] {
    case "execute":
      guard let request = readRequest() else { Darwin.exit(2) }
      Darwin.exit(execute(request))
    case "cancel":
      guard let request = readRequest() else { Darwin.exit(2) }
      Darwin.exit(cancel(request))
    case "start-and-exit":
      guard let request = readRequest() else { Darwin.exit(2) }
      startAndExit(request)
    case "probe":
      guard arguments.count == 3 else { Darwin.exit(2) }
      guard let readable = ProofClient().canReadHostFixture(arguments[2]) else {
        writeJSON(["status": "failed"])
        Darwin.exit(1)
      }
      writeJSON(["status": readable ? "readable" : "denied"])
      Darwin.exit(readable ? 1 : 0)
    default:
      Darwin.exit(2)
    }
  }
}
