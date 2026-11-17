import Foundation

extension NativeClient {
    /// Discover contracts with Swift Task cancellation wired to the Rust token.
    /// Up to three round trips, bounded response buffering and O(result bytes) copying;
    /// no local persistence. Throws NativeError; cancellation cannot undo server effects.
    public func describe() async throws -> String {
        let token = NativeCancellation()
        return try await withTaskCancellationHandler {
            try await describe(cancellation: token)
        } onCancel: {
            token.cancel()
        }
    }

    /// Resolve under current authority with Swift Task cancellation. Up to three round
    /// trips and bounded O(result bytes) copying, no local persistence. Throws NativeError;
    /// missing and inaccessible items share the server's not-found response.
    public func resolve(kind: String, id: String) async throws -> String {
        let token = NativeCancellation()
        return try await withTaskCancellationHandler {
            try await resolve(kind: kind, id: id, cancellation: token)
        } onCancel: {
            token.cancel()
        }
    }

    /// Execute a retained request with Swift Task cancellation; cancellation leaves a
    /// mutation outcome uncertain, so retain the same request for reconciliation.
    /// Up to three round trips, O(input/result bytes) copies and bounded buffering;
    /// no local persistence. Throws NativeError without exposing the session credential.
    public func call(request: NativeRequest) async throws -> String {
        let token = NativeCancellation()
        return try await withTaskCancellationHandler {
            try await call(request: request, cancellation: token)
        } onCancel: {
            token.cancel()
        }
    }
}
