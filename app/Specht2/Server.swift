//
//  Server.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/16.
//

import Foundation
import Defaults

// We are not using `actor` since it won't work with synchronous code (other than create a `Task`).
// Before Apple makes AppKit fully async-ready, we cannot use the new fancy concurrecy feature with anything
// but the very limited SwiftUI.

// We don't really care the cycle reference here since the server instance is expected
// to exist throught the app's lifetime.
//
// Call `stop()` can properly release all cycled reference.
class Server {
    private var stopHandle: StopHandle?
    private var afterStop: (() -> Void)?
    private let queue = DispatchQueue(label: "me.zhuhaow.Specht2")

    private var running: Bool {
        self.stopHandle != nil
    }

    func run(name: String, configUrl: URL, stopCallback: @escaping (String?) -> Void) {
        queue.async {
            if self.running {
                self.afterStop = {
                    self.runServer(name: name, configUrl: configUrl, stopCallback: stopCallback)
                }

                self.stop()
            } else {
                self.runServer(name: name, configUrl: configUrl, stopCallback: stopCallback)
            }
        }
    }

    private func runServer(name: String, configUrl: URL, stopCallback: @escaping (String?) -> Void) {
        // We only clear it here, so every new start of server will never use old callback.
        afterStop = nil
        stopHandle = startServer(configUrl: configUrl) { err in
            self.stopHandle = nil

            stopCallback(err)

            self.afterStop?()
        }
    }

    private func stop() {
        stopHandle = nil
    }

    func shutdown() {
        queue.async {
            self.afterStop = nil
            self.stopHandle = nil
        }
    }

    func isRunning() -> Bool {
        return queue.sync {
            return self.running
        }
    }
}

private class StopHandle {
    let handle: UnsafeMutableRawPointer

    init(handle: UnsafeMutableRawPointer) {
        self.handle = handle
    }

    deinit {
        specht2_stop(handle)
    }
}

private func startServer(configUrl: URL, closure: @escaping (String?) -> Void) -> StopHandle {
    let wrappedClosure = WrapClosure(closure: closure)
    let data = Unmanaged.passRetained(wrappedClosure).toOpaque()

    let callback: @convention(c) (UnsafeMutableRawPointer, UnsafePointer<CChar>?) -> Void
        = { (_ userdata: UnsafeMutableRawPointer, _ err: UnsafePointer<CChar>?) in
            let wrappedClosure: WrapClosure<(String?) -> Void> = Unmanaged.fromOpaque(userdata).takeRetainedValue()

            guard let err = err else {
                wrappedClosure.closure(nil)
                return
            }

            // The conversion should never fail thus we don't check nil here
            wrappedClosure.closure(String.init(cString: err, encoding: .utf8))
        }

    let completion = CompletedCallback(userdata: data, callback: callback)

    return StopHandle(handle: configUrl.withUnsafeFileSystemRepresentation {
        // Due to the limit of cbindgen, the generated signiture for NonNull is non-const,
        // thus the pointer has to be mutable.
        // But we won't mutate it anywhere.
        // The $0 should not be nil according to the docs since the path should always be
        // possible to have a valid representation.
        let mutablePtr = UnsafeMutablePointer(mutating: $0!)
        return specht2_start(mutablePtr, completion)
    })
}

private class WrapClosure<T> {
    let closure: T

    init(closure: T) {
        self.closure = closure
    }
}