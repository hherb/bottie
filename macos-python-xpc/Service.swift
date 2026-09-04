import Darwin
import Foundation

private let allowedIdentifierCharacters = CharacterSet(
  charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789-")
private let maximumIdentifierCharacters = 64
private let forcedTerminationDelay: TimeInterval = 1

/// Thread-safe capture for the runner's bounded stdout and discarded stderr streams.
private final class CapturedOutput {
  private let lock = NSLock()
  private var value = Data()

  /// Replaces the captured value after one pipe reaches end of file.
  func store(_ data: Data) {
    lock.lock()
    value = data
    lock.unlock()
  }

  /// Returns an immutable copy of the captured bytes.
  func load() -> Data {
    lock.lock()
    defer { lock.unlock() }
    return value
  }
}

/// One private-pipe child process retained until it reaches a terminal state.
private final class RunningExecution {
  let captureGroup: DispatchGroup
  let process: Process
  let stdout: CapturedOutput
  var cancelled = false

  init(captureGroup: DispatchGroup, process: Process, stdout: CapturedOutput) {
    self.captureGroup = captureGroup
    self.process = process
    self.stdout = stdout
  }
}

/// Path-free terminal result retained until the proof client polls it once.
private struct FinishedExecution {
  let state: String
  let result: Data?
  let error: String?
}

/// Per-connection execution state; invalidating the client kills every retained runner.
@objcMembers
private final class PythonRunnerService: NSObject, BottiePythonRunnerServiceProtocol {
  private enum ExecutionState {
    case running(RunningExecution)
    case finished(FinishedExecution)
  }

  private let queue = DispatchQueue(label: "com.bottie.python-runner.state")
  private var executions: [String: ExecutionState] = [:]

  /// Starts the exact Rust runner behind private stdin, stdout, and stderr pipes.
  func start(
    _ request: Data,
    identifier: String,
    withReply reply: @escaping (NSNumber?, String?) -> Void
  ) {
    queue.async { [weak self] in
      guard let self else { return reply(nil, "service_unavailable") }
      guard Self.validIdentifier(identifier), !request.isEmpty, request.count <= maximumRequestBytes
      else {
        return reply(nil, "invalid_request")
      }
      guard self.executions[identifier] == nil else { return reply(nil, "duplicate_identifier") }

      do {
        let execution = try Self.launch(request: request) { [weak self] execution in
          guard let self else { return }
          execution.captureGroup.notify(queue: self.queue) {
            self.finish(execution, identifier: identifier)
          }
        }
        self.executions[identifier] = .running(execution)
        reply(NSNumber(value: execution.process.processIdentifier), nil)
      } catch {
        reply(nil, "launch_failed")
      }
    }
  }

  /// Returns running state or consumes one terminal transport result.
  func poll(
    _ identifier: String,
    withReply reply: @escaping (String, Data?, String?) -> Void
  ) {
    queue.async { [weak self] in
      guard let self else { return reply("failed", nil, "service_unavailable") }
      switch self.executions[identifier] {
      case .running:
        reply("running", nil, nil)
      case .finished(let finished):
        self.executions.removeValue(forKey: identifier)
        reply(finished.state, finished.result, finished.error)
      case nil:
        reply("failed", nil, "unknown_identifier")
      }
    }
  }

  /// Sends a bounded termination signal and records cancellation before process completion.
  func cancel(_ identifier: String, withReply reply: @escaping (Bool) -> Void) {
    queue.async { [weak self] in
      guard
        let self,
        case .running(let execution)? = self.executions[identifier]
      else {
        return reply(false)
      }
      execution.cancelled = true
      execution.process.terminate()
      Self.forceTerminationIfNeeded(execution.process)
      reply(true)
    }
  }

  /// Attempts a direct service-process read to prove the outer App Sandbox denial.
  func canReadHostFixture(_ path: String, withReply reply: @escaping (Bool) -> Void) {
    queue.async {
      guard !path.isEmpty else { return reply(false) }
      reply((try? Data(contentsOf: URL(fileURLWithPath: path))) != nil)
    }
  }

  /// Immediately kills every child when the owning XPC connection disappears.
  func cancelAllForInvalidatedConnection() {
    queue.sync {
      for state in executions.values {
        guard case .running(let execution) = state, execution.process.isRunning else { continue }
        execution.cancelled = true
        Darwin.kill(execution.process.processIdentifier, SIGKILL)
      }
    }
  }

  private static func validIdentifier(_ identifier: String) -> Bool {
    let scalars = identifier.unicodeScalars
    return !scalars.isEmpty
      && scalars.count <= maximumIdentifierCharacters
      && scalars.allSatisfy(allowedIdentifierCharacters.contains)
  }

  private static func launch(
    request: Data,
    onTermination: @escaping (RunningExecution) -> Void
  ) throws -> RunningExecution {
    let serviceBundle = Bundle.main.bundleURL
    let runner = serviceBundle.appendingPathComponent("Contents/Helpers/bottie-python-runner")
    let runtime = serviceBundle.appendingPathComponent(
      "Contents/Resources/python-runtime", isDirectory: true)
    let stdinPipe = Pipe()
    let stdoutPipe = Pipe()
    let stderrPipe = Pipe()
    let process = Process()
    let captureGroup = DispatchGroup()
    process.executableURL = runner
    process.arguments = ["--runtime", runtime.path]
    process.environment = [:]
    process.standardInput = stdinPipe
    process.standardOutput = stdoutPipe
    process.standardError = stderrPipe
    let captured = CapturedOutput()
    captureGroup.enter()
    captureGroup.enter()
    let execution = RunningExecution(captureGroup: captureGroup, process: process, stdout: captured)
    process.terminationHandler = { [weak execution] _ in
      if let execution {
        onTermination(execution)
      }
    }

    try process.run()

    DispatchQueue.global(qos: .userInitiated).async {
      try? stdinPipe.fileHandleForWriting.write(contentsOf: request)
      try? stdinPipe.fileHandleForWriting.close()
    }
    DispatchQueue.global(qos: .userInitiated).async {
      captured.store(stdoutPipe.fileHandleForReading.readDataToEndOfFile())
      captureGroup.leave()
    }
    DispatchQueue.global(qos: .utility).async {
      _ = stderrPipe.fileHandleForReading.readDataToEndOfFile()
      captureGroup.leave()
    }
    return execution
  }

  private func finish(_ execution: RunningExecution, identifier: String) {
    guard case .running(let current)? = executions[identifier], current === execution else {
      return
    }
    if execution.cancelled {
      executions[identifier] = .finished(
        FinishedExecution(state: "cancelled", result: nil, error: nil))
      return
    }
    let result = execution.stdout.load()
    guard execution.process.terminationStatus == 0, !result.isEmpty,
      result.count <= maximumResponseBytes
    else {
      executions[identifier] = .finished(
        FinishedExecution(state: "failed", result: nil, error: "runner_failed")
      )
      return
    }
    executions[identifier] = .finished(
      FinishedExecution(state: "completed", result: result, error: nil))
  }

  private static func forceTerminationIfNeeded(_ process: Process) {
    DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + forcedTerminationDelay) {
      if process.isRunning {
        Darwin.kill(process.processIdentifier, SIGKILL)
      }
    }
  }
}

/// Accepts only private connections launched for the enclosing application.
private final class ServiceDelegate: NSObject, NSXPCListenerDelegate {
  func listener(_ listener: NSXPCListener, shouldAcceptNewConnection connection: NSXPCConnection)
    -> Bool
  {
    let service = PythonRunnerService()
    connection.exportedInterface = NSXPCInterface(with: BottiePythonRunnerServiceProtocol.self)
    connection.exportedObject = service
    connection.invalidationHandler = { [weak service] in
      service?.cancelAllForInvalidatedConnection()
    }
    connection.interruptionHandler = { [weak service] in
      service?.cancelAllForInvalidatedConnection()
    }
    connection.resume()
    return true
  }
}

/// Entrypoint for the separately signed private XPC service.
@main
private struct ServiceMain {
  static func main() {
    let delegate = ServiceDelegate()
    let listener = NSXPCListener.service()
    listener.delegate = delegate
    listener.resume()
    RunLoop.current.run()
  }
}
