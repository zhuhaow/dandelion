//
//  Server.swift
//  Specht2
//
//  Created by Zhuhao Wang on 2021/12/16.
//

import Foundation
import Defaults

// We don't really care the cycle reference here since the server instance is expected
// to exist throught the app's lifetime.
//
// Call `stop()` can properly release all cycled reference.
actor Server {
    private var stopHandle: StopHandle?
    private var afterStop: (() -> Void)?

    var running: Bool {
        stopHandle != nil
    }

    func run(name: String, configUrl: URL, stopCallback: @escaping (String?) -> Void) {
        if running {
            afterStop = {
                self.runServer(name: name, configUrl: configUrl, stopCallback: stopCallback)
            }

            stop()
        } else {
            runServer(name: name, configUrl: configUrl, stopCallback: stopCallback)
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
        afterStop = nil
        stopHandle = nil
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
