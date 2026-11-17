import Foundation

extension NativeLive {
    /// Adapt the Rust pull interface into an async stream of JSON events. The
    /// consumer applies each event, persists its bookmark and calls accept before
    /// processing the next. One buffered event is permitted; overflow closes with
    /// an error so reconnect replays from the last accepted bookmark. Pass the
    /// same cancellation used when subscribing. No UI state crosses this boundary.
    public func frames(cancellation: NativeCancellation) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream(bufferingPolicy: .bufferingOldest(1)) { continuation in
            let task = Task {
                do {
                    while !Task.isCancelled {
                        let event = try await self.next()
                        switch continuation.yield(event) {
                        case .enqueued: break
                        case .dropped:
                            cancellation.cancel()
                            continuation.finish(throwing: NativeError.Failure(details: "live consumer fell behind; reconnect from accepted bookmark"))
                            return
                        case .terminated: return
                        @unknown default: return
                        }
                    }
                } catch { continuation.finish(throwing: error) }
            }
            continuation.onTermination = { _ in
                cancellation.cancel()
                task.cancel()
            }
        }
    }
}
